/** QCY BLE GATT UUIDs from public reverse-engineering of the QCY earphone protocol.
 *  Independent documentation; not copied from any proprietary SDK.
 *  Base: Bluetooth SIG 16-bit UUID in the 0000xxxx-0000-1000-8000-00805f9b34fb range.
 */

export const BT_BASE = "0000-1000-8000-00805f9b34fb";

export function uuid16(short: string): string {
  const n = short.replace(/^0x/i, "").padStart(4, "0").toLowerCase();
  return `0000${n}-${BT_BASE}`;
}

export const QCY_COMPANY_ID = 0x521c;
export const QCY_WATCH_COMPANY_ID = 0x05d6;

export const SERVICE = {
  main: uuid16("a001"),
} as const;

export const CHAR = {
  leftSingleTapV1: uuid16("0001"),
  rightSingleTapV1: uuid16("0002"),
  leftDoubleTapV1: uuid16("0003"),
  rightDoubleTapV1: uuid16("0004"),
  leftTripleTapV1: uuid16("0005"),
  rightTripleTapV1: uuid16("0006"),
  version: uuid16("0007"),
  battery: uuid16("0008"),
  language: uuid16("0009"),
  resetV1: uuid16("000a"),
  eqDirect: uuid16("000b"),
  sendTimeV1: uuid16("000c"),
  keyFunctionV2: uuid16("000d"),
  zrSettings: uuid16("000e"),
  inEarCheckJl: uuid16("000f"),
  commandWrite: uuid16("1001"),
  settingsNotify: uuid16("1002"),
  unknown1003: uuid16("1003"),
} as const;

export const STD = {
  cccd: uuid16("2902"),
  batteryLevel: uuid16("2a19"),
  modelNumber: uuid16("2a24"),
  serialNumber: uuid16("2a25"),
  firmwareRevision: uuid16("2a26"),
  hardwareRevision: uuid16("2a27"),
  softwareRevision: uuid16("2a28"),
  manufacturerName: uuid16("2a29"),
  pnpId: uuid16("2a50"),
} as const;

export const DIRECT_WRITE_CHARS = new Set<string>([CHAR.eqDirect, CHAR.keyFunctionV2]);
