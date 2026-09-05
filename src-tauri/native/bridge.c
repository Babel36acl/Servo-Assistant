/* GPL-3.0-only. Stable application ABI around the pinned SOEM source.
 * All master entry points are serialized by Rust's master mutex.
 * Npcap is loaded exclusively from the system Npcap installation. */
#include <winsock2.h>
#include <windows.h>
#include <stdint.h>
#include <stdio.h>
#include "ethercat.h"

typedef void (*capture_fn)(int, const unsigned char *, unsigned int, uint64_t);
static capture_fn capture;
static HMODULE pcap_module;
static INIT_ONCE pcap_once = INIT_ONCE_STATIC_INIT;
static pcap_t *master_socket;
static int opened;
static pcap_t *(*p_open)(const char*, int, int, int, struct pcap_rmtauth*, char*);
static void (*p_close)(pcap_t*);
static int (*p_send)(pcap_t*, const u_char*, int);
static int (*p_next)(pcap_t*, struct pcap_pkthdr**, const u_char**);
static int (*p_find)(pcap_if_t**, char*);
static void (*p_free)(pcap_if_t*);
static int (*p_stats)(pcap_t*, struct pcap_stat*);
static int (*p_link)(pcap_t*);
static int (*p_nonblock)(pcap_t*, int, char*);
static BOOL CALLBACK load_pcap(PINIT_ONCE once, PVOID param, PVOID *context) {
    wchar_t path[MAX_PATH]; UINT length;
    (void)once; (void)param; (void)context;
    length = GetSystemDirectoryW(path, MAX_PATH);
    if (!length || length + 18 >= MAX_PATH) return TRUE;
    wcscat_s(path, MAX_PATH, L"\\Npcap\\wpcap.dll");
    pcap_module = LoadLibraryExW(path, NULL, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32);
    if (!pcap_module) return TRUE;
#define LOAD(field, name) field = (void*)GetProcAddress(pcap_module, name); if (!field) { FreeLibrary(pcap_module); pcap_module = NULL; return TRUE; }
    LOAD(p_open, "pcap_open") LOAD(p_close, "pcap_close") LOAD(p_send, "pcap_sendpacket")
    LOAD(p_next, "pcap_next_ex") LOAD(p_find, "pcap_findalldevs") LOAD(p_free, "pcap_freealldevs")
    LOAD(p_stats, "pcap_stats") LOAD(p_link, "pcap_datalink") LOAD(p_nonblock, "pcap_setnonblock")
    return TRUE;
}
int sa_available(void) { InitOnceExecuteOnce(&pcap_once, load_pcap, NULL, NULL); return pcap_module != NULL; }
/* These symbols satisfy SOEM's pcap imports, without a load-time driver dependency. */
pcap_t *pcap_open(const char *name, int snap, int flags, int timeout, struct pcap_rmtauth *auth, char *error) {
    pcap_t *p;
    if (!sa_available()) { strcpy_s(error, PCAP_ERRBUF_SIZE, "Npcap is not installed; restart after installation"); return NULL; }
    p = p_open(name, snap, flags, timeout, auth, error);
    if (p && (p_link(p) != 1 || p_nonblock(p, 1, error) < 0)) { p_close(p); return NULL; }
    master_socket = p;
    return p;
}
void pcap_close(pcap_t *p) { if (p) p_close(p); if (p == master_socket) master_socket = NULL; }
int pcap_sendpacket(pcap_t *p, const u_char *bytes, int size) {
    int result = p_send(p, bytes, size);
    if (capture && size > 0) capture(result == 0 ? 1 : 3, bytes, (unsigned int)size, 0);
    return result;
}
int pcap_next_ex(pcap_t *p, struct pcap_pkthdr **h, const u_char **bytes) {
    int result = p_next(p, h, bytes);
    if (result == 1 && capture) capture(2, *bytes, (*h)->caplen, (uint64_t)(*h)->ts.tv_sec * 1000000 + (*h)->ts.tv_usec);
    if (result == 1) {
        /* Reject truncated or malformed frames before SOEM's fixed buffers see them.
         * The raw callback above still preserves the rejected observation. */
        unsigned int cap = (*h)->caplen, end, pos;
        const unsigned char *b = *bytes;
        if (cap < 28 || cap != (*h)->len || cap > EC_MAXECATFRAME || b[12] != 0x88 || b[13] != 0xa4) return 0;
        end = 16 + ((b[14] | ((unsigned int)b[15] << 8)) & 0x7ff);
        if (end > cap || (b[15] >> 4) != 1 || (b[15] & 8)) return 0;
        pos = 16;
        while (pos < end) {
            unsigned int count, next;
            if (end - pos < 12) return 0;
            count = (b[pos+6] | ((unsigned int)b[pos+7] << 8)) & 0x7ff;
            next = pos + 12 + count;
            if (next > end || ((b[pos+7] & 0x80) ? next == end : next != end)) return 0;
            pos = next;
        }
    }
    return result;
}
int pcap_findalldevs(pcap_if_t **devices, char *error) {
    if (!sa_available()) { *devices = NULL; strcpy_s(error, PCAP_ERRBUF_SIZE, "Npcap unavailable"); return -1; }
    return p_find(devices, error);
}
void pcap_freealldevs(pcap_if_t *devices) { if (devices) p_free(devices); }
int sa_adapters(char *out, int capacity) {
    pcap_if_t *devices = NULL, *d; char error[PCAP_ERRBUF_SIZE]; int used = 0;
    if (pcap_findalldevs(&devices, error) < 0) return -1;
    for (d = devices; d; d = d->next) {
        int n = snprintf(out + used, capacity - used, "%s\t%s\n", d->name, d->description ? d->description : d->name);
        if (n < 0 || n >= capacity - used) { pcap_freealldevs(devices); return -2; }
        used += n;
    }
    pcap_freealldevs(devices); return used;
}
void sa_close(void) { if (opened) { ec_close(); opened = 0; } capture = NULL; }
int sa_open(const char *adapter, capture_fn callback) {
    int n;
    if (opened) return -2;
    capture = callback;
    if (!ec_init(adapter)) { capture = NULL; return -1; }
    opened = 1;
    n = ec_config_init(FALSE); /* Mailbox setup; remains PRE-OP. No PDO outputs, no OP request. */
    if (n <= 0) { sa_close(); return 0; }
    ec_statecheck(0, EC_STATE_PRE_OP, EC_TIMEOUTSTATE);
    return n;
}
/* Fixed-width fields, no exposure of SOEM's internal struct layout to Rust. */
typedef struct {
    uint32_t vendor, product, revision;
    uint16_t position, station, state, al_code, mailbox_out, mailbox_in, mailbox_protocols;
    char name[42];
} sa_slave;
int sa_slaves(sa_slave *out, int capacity) {
    int i;
    if (!opened) return -1;
    if (ec_readstate() == EC_STATE_NONE) return -2;
    if (capacity < ec_slavecount) return -3;
    for (i = 1; i <= ec_slavecount; i++) {
        sa_slave *s = &out[i-1]; memset(s, 0, sizeof(*s));
        s->position = (uint16_t)i; s->station = ec_slave[i].configadr;
        s->vendor = ec_slave[i].eep_man; s->product = ec_slave[i].eep_id; s->revision = ec_slave[i].eep_rev;
        s->state = ec_slave[i].state; s->al_code = ec_slave[i].ALstatuscode;
        s->mailbox_out = ec_slave[i].mbx_wo; s->mailbox_in = ec_slave[i].mbx_ro; s->mailbox_protocols = ec_slave[i].mbx_proto;
        memcpy(s->name, ec_slave[i].name, EC_MAXNAME + 1);
    }
    return ec_slavecount;
}
int sa_sdo_read(uint16_t slave, uint16_t index, uint8_t sub, unsigned char *out, int *size) {
    if (!opened || slave < 1 || slave > ec_slavecount) return -1;
    return ec_SDOread(slave, index, sub, FALSE, size, out, 500000);
}
int sa_sdo_write(uint16_t slave, uint16_t index, uint8_t sub, const unsigned char *data, int size) {
    if (!opened || slave < 1 || slave > ec_slavecount || size < 1 || size > 4096) return -1;
    return ec_SDOwrite(slave, index, sub, FALSE, size, (void*)data, 500000);
}
int sa_error(char *out, int capacity) {
    int used = 0;
    while (ec_iserror()) {
        const char *message = ec_elist2string();
        if (used < capacity-1) { int n = snprintf(out+used, capacity-used, "%s", message); if (n > 0) used += n < capacity-used ? n : capacity-used-1; }
    }
    return used;
}
int sa_drops(void) { struct pcap_stat s; return master_socket && p_stats(master_socket, &s) == 0 ? (int)s.ps_drop : -1; }
/* A separate passive handle, exclusively owned by one capture worker. */
void *sa_capture_open(const char *adapter, char *error) {
    pcap_t *p;
    if (!sa_available()) { strcpy_s(error, PCAP_ERRBUF_SIZE, "Npcap unavailable"); return NULL; }
    p = p_open(adapter, 65536, PCAP_OPENFLAG_PROMISCUOUS | PCAP_OPENFLAG_MAX_RESPONSIVENESS, 50, NULL, error);
    if (p && (p_link(p) != 1 || p_nonblock(p, 1, error) < 0)) { p_close(p); strcpy_s(error, PCAP_ERRBUF_SIZE, "Ethernet capture required"); return NULL; }
    return p;
}
int sa_capture_next(void *handle, unsigned char *out, unsigned int capacity, uint64_t *stamp, unsigned int *original) {
    struct pcap_pkthdr *h; const u_char *data; int result = p_next(handle, &h, &data);
    if (result != 1) return result == 0 ? 0 : -1;
    if (h->caplen > capacity) return -2;
    memcpy(out, data, h->caplen); *stamp = (uint64_t)h->ts.tv_sec * 1000000 + h->ts.tv_usec; *original = h->len;
    return (int)h->caplen;
}
int sa_capture_drops(void *handle) { struct pcap_stat s; return p_stats(handle, &s) == 0 ? (int)s.ps_drop : -1; }
void sa_capture_close(void *handle) { p_close(handle); }
