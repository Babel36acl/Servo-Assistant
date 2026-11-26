//! 主站单元测试：对象编解码、CoE 映射与写前确认/回读路径。
use super::coe::coe_object;
use super::*;
fn object() -> Object {
    Object {
        id: "gain".into(),
        name: "Gain".into(),
        index: 0x2000,
        sub_index: 0,
        size: 2,
        signed: true,
        writable: true,
        min: "-32768".into(),
        max: "32767".into(),
        scale: 0.1,
        unit: "".into(),
    }
}
#[test]
fn coe_object_keeps_supported_integers_and_skips_other_types() {
    let gain = coe_object(0x2000, 0, "Gain", 0x0003, 16, 0x03).unwrap();
    assert_eq!(gain.id, "2000:00");
    assert_eq!(gain.name, "Gain");
    assert_eq!(gain.size, 2);
    assert!(gain.signed);
    assert!(gain.writable);
    assert_eq!((gain.min.as_str(), gain.max.as_str()), ("-32768", "32767"));
    let counter = coe_object(0x2001, 1, "", 0x0007, 32, 0x01).unwrap();
    assert_eq!(counter.name, "对象 2001:01");
    assert_eq!(counter.size, 4);
    assert!(!counter.signed);
    assert!(!counter.writable);
    assert_eq!(counter.max, "4294967295");
    let big = coe_object(0x2002, 0, "Big", 0x0014, 64, 0x01).unwrap();
    assert_eq!(big.min, "-9223372036854775808");
    assert!(coe_object(0x2003, 0, "Float", 0x0008, 32, 0x03).is_none());
    assert!(coe_object(0x2004, 0, "Text", 0x0009, 0, 0x01).is_none());
    assert!(coe_object(0x2005, 0, "Bit", 0x0001, 1, 0x01).is_none());
    assert!(coe_object(0x2006, 0, "Odd", 0x0002, 24, 0x01).is_none());
}
#[test]
fn signed_ranges_and_precision() {
    let o = object();
    assert_eq!(o.encode("-2").unwrap(), [254, 255]);
    assert_eq!(o.value(&[254, 255]).unwrap(), "-2");
    assert!(o.encode("32768").is_err());
    assert!(o.encode("1.5").is_err());
    assert!(o.value(&[1]).is_err());
}
#[test]
fn duplicate_contract_rejected() {
    let o = object();
    let p = MasterProfile {
        schema_version: "ethercat-1.0".into(),
        name: "test".into(),
        vendor: 1,
        product: 2,
        objects: vec![o.clone(), o],
    };
    assert!(p.validate().is_err());
}
fn reading(raw: &str) -> ObjectValue {
    ObjectValue {
        id: "gain".into(),
        raw: raw.into(),
        value: 0.0,
        timestamp_us: 0,
    }
}
#[test]
fn stale_and_unauthorized_never_write() {
    let o = object();
    assert!(verified_write(
        &o,
        "3",
        "1",
        "写入 gain",
        || Ok(reading("2")),
        |_| panic!("stale write")
    )
    .is_err());
    assert!(verified_write(
        &o,
        "3",
        "1",
        "wrong",
        || panic!("unauthorized read"),
        |_| panic!("unauthorized write")
    )
    .is_err());
}
#[test]
fn one_write_and_readback_mismatch_is_not_retried() {
    let o = object();
    let writes = std::cell::Cell::new(0);
    let reads = std::cell::Cell::new(0);
    let result = verified_write(
        &o,
        "3",
        "1",
        "写入 gain",
        || {
            reads.set(reads.get() + 1);
            Ok(reading(if reads.get() == 1 { "1" } else { "2" }))
        },
        |bytes| {
            assert_eq!(bytes, [3, 0]);
            writes.set(writes.get() + 1);
            Ok(())
        },
    );
    assert!(result.unwrap_err().contains("回读不一致"));
    assert_eq!(writes.get(), 1);
    assert_eq!(reads.get(), 2);
}
#[test]
fn transport_failure_stops_without_readback_or_retry() {
    let o = object();
    let reads = std::cell::Cell::new(0);
    let result = verified_write(
        &o,
        "3",
        "1",
        "写入 gain",
        || {
            reads.set(reads.get() + 1);
            Ok(reading("1"))
        },
        |_| Err("timeout".into()),
    );
    assert!(result.unwrap_err().contains("未知"));
    assert_eq!(reads.get(), 1);
}
