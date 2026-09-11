"""Offline protocol metadata from actual packet captures; no interfaces are opened."""
import json
from common import result, run_tool
FIELDS = ['wpan.src16','wpan.dst16','wpan.src_pan','wpan.dst_pan','wpan.security','frame.number','frame.protocols','wlan.fc.type_subtype','wlan.sa','wlan.da','wlan.ssid','wlan.fc.protected',
          'btle.advertising_address','btle.access_address','btcommon.eir_ad.entry.device_name',
          'btcommon.eir_ad.entry.uuid_16','btcommon.cod.class_of_device','btatt.uuid16','bthci_evt.encryption_enable']

def capture(name,request):
    text,log,truncated=run_tool(['tshark','-n','-r',request['input']['path'],'-c','2000','-T','json',
                                *[item for field in FIELDS for item in ['-e',field]]],timeout=30)
    packets=json.loads(text or '[]') if not truncated else []
    if truncated: raise ValueError('packet JSON exceeded output limit; supply a smaller capture')
    events=[];candidates=[]
    for packet in packets:
        layers=packet.get('_source',{}).get('layers',{})
        protocols=':'.join(layers.get('frame.protocols',[]))
        wlan='wlan' in protocols
        bt=any(p in protocols for p in ['btle','bthci','btl2cap','btatt','bthid','btbredr'])
        if name=='zigbee-pcap' and 'wpan' in protocols:
            events.append(dict(protocol='IEEE 802.15.4 / ZigBee' if 'zbee' in protocols else 'IEEE 802.15.4',evidence='dissected packet capture',fields=layers))
        elif name in ('wifi-id','wifi-pcap') and wlan:
            events.append(dict(protocol='802.11',evidence='dissected packet capture',fields=layers))
        elif name in ('bluetooth-id','bluetooth-pcap') and bt:
            events.append(dict(protocol='Bluetooth/BLE',evidence='dissected packet capture',fields=layers))
        elif name=='hid-id':
            uuids=layers.get('btcommon.eir_ad.entry.uuid_16',[])+layers.get('btatt.uuid16',[])
            if any(v.lower() in ('0x1812','0x1124','6162','4388') for v in uuids) or 'bthid' in protocols:
                candidates.append(dict(label='Bluetooth HID service',confidence='protocol-evidence',evidence='HID UUID / dissected HID protocol',fields=layers))
            for value in layers.get('btcommon.cod.class_of_device',[]):
                cod=int(value,0)
                if (cod>>8)&31==5:
                    kind='keyboard and pointing device' if cod&0xc0==0xc0 else 'keyboard' if cod&0x40 else 'pointing device' if cod&0x80 else 'peripheral'
                    candidates.append(dict(label='Bluetooth '+kind,confidence='advertised-device-class',evidence=f'Class of Device {value}; self-reported by device'))
        elif name=='encryption-id':
            if any(v in ('1','true','True') for v in layers.get('wlan.fc.protected',[])):
                candidates.append(dict(label='802.11 protected frame',confidence='protocol-evidence',evidence='Protected bit set; does not establish cipher strength or successful decryption',frame=layers.get('frame.number')))
            if any(v not in ('0','0x00') for v in layers.get('bthci_evt.encryption_enable',[])):
                candidates.append(dict(label='Bluetooth link encryption enabled',confidence='protocol-evidence',evidence='HCI Encryption Change event',frame=layers.get('frame.number')))
    return result(name,events=events[:100],candidates=candidates[:100],diagnostics=log,
                  notes=['Reads at most 2000 existing packets; no RF demodulation, active scanning or decryption. Advertised names/classes are untrusted claims.'])
