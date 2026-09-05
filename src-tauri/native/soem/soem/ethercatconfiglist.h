/*
 * Licensed under the GNU General Public License version 2 with exceptions. See
 * LICENSE file in the project root for full license information
 */

/** \file
 * \brief
 * DEPRECATED Configuration list of known EtherCAT slave devices.
 *
 * If a slave is found in this list it is configured according to the parameters
 * in the list. Otherwise the configuration info is read directly from the slave
 * EEPROM (SII or Slave Information Interface).
 */

#ifndef _ethercatconfiglist_
#define _ethercatconfiglist_

#ifdef __cplusplus
extern "C"
{
#endif

/* Servo Assistant removes the deprecated device table. Configuration is read
 * from each slave's SII; application object definitions come from user profiles. */
#define EC_CONFIGEND 0xffffffff

ec_configlist_t ec_configlist[] = {
    {0},
    {EC_CONFIGEND}
};

#ifdef __cplusplus
}
#endif

#endif
