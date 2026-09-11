"""Evidence-based signal families and sourced, offline allocation context."""
import numpy as np
from scipy import signal
from common import load_samples, result, channel

SOURCES = {
    'efis':'https://efis.cept.org/',
    'gsm':'https://www.3gpp.org/technologies/gsm',
    'ble':'https://www.bluetooth.com/specifications/specs/core-specification/',
    'ais':'https://www.itu.int/rec/R-REC-M.1371/en',
    'atis':'https://www.itu.int/rec/R-REC-M.493/en',
    'gps':'https://www.gps.gov/technical-documentation',
    'wifi':'https://standards.ieee.org/ieee/802.11/7028/',
    'sat':'https://www.satdump.org/',
    'tv':'https://dvb.org/standards/',
}
# Region/context hints, not assertions about current transmitter activity.
BANDS = {
    'paging-id': [(138,174,'VHF paging / land mobile; try POCSAG or FLEX','efis'),(450,470,'UHF paging / land mobile; try POCSAG or FLEX','efis')],
    'maritime-id': [(161.962,161.988,'AIS A channel vicinity','ais'),(162.012,162.038,'AIS B channel vicinity','ais'),(156.50,156.55,'Marine DSC channel 70 vicinity','atis'),(156,162.05,'Marine VHF voice / ATIS candidate','atis')],
    'aviation-atis-id': [(117.975,137,'Aviation VHF AM; ATIS is one possible voice service','efis')],
    'cellular-id': [(880,915,'E-GSM 900 uplink allocation context','gsm'),(925,960,'E-GSM 900 downlink allocation context','gsm'),(1710,1785,'DCS 1800 uplink allocation context','gsm'),(1805,1880,'DCS 1800 downlink allocation context','gsm')],
    'wifi-id': [(2400,2483.5,'2.4 GHz WLAN / Bluetooth / other ISM users','wifi'),(5150,5875,'5 GHz WLAN allocation context (regional restrictions apply)','wifi')],
    'bluetooth-id': [(2400,2483.5,'Bluetooth / BLE shares the 2.4 GHz band','ble')],
    'hid-id': [(2400,2483.5,'Wireless peripherals can use this band, as can many unrelated devices','ble'),(26.8,27.3,'Some legacy peripherals share 27 MHz with other services','efis')],
    'walkie-id': [(446,446.2,'European PMR446 channel range; analog FM or digital','efis'),(144,146,'2 m amateur radio; handhelds are one possible source','efis'),(430,440,'70 cm amateur radio; handhelds are one possible source','efis')],
    'weather-id': [(433.05,434.79,'433 MHz SRD sensors; try rtl_433 weather decoder','efis'),(863,870,'European SRD sensors; try rtl_433','efis'),(400,406,'Meteorological aids / radiosonde context','efis')],
    'tv-id': [(174,230,'VHF broadcast allocation context: DVB-T/T2 or DAB by region','tv'),(470,694,'European UHF terrestrial television allocation context','tv')],
    'satellite-id': [(137,138,'Meteorological satellite VHF allocation; verify active satellite/service','sat'),(145.8,146,'Amateur satellite VHF allocation context','efis'),(400.15,401,'Meteorological satellite UHF allocation context','sat'),(1690,1710,'Weather satellite L-band allocation context','sat'),(1525,1559,'Mobile satellite L-band downlink context','sat')],
    'gnss-id': [(1573.42,1577.42,'GPS L1 / Galileo E1 / other GNSS vicinity','gps'),(1225.60,1229.60,'GPS L2 vicinity','gps'),(1174.45,1178.45,'GPS L5 / Galileo E5a vicinity','gps')],
}

def band_candidates(name, request):
    if request['input']['format'] == 'wav': return []
    mhz = request['input']['center_hz']/1e6
    return [dict(label=label, confidence='frequency-context-only', evidence=f'capture center {mhz:.6f} MHz falls within {lo}–{hi} MHz', source=SOURCES[src])
            for lo,hi,label,src in BANDS.get(name,[]) if lo<=mhz<=hi]

def bursts(request):
    samples, rate = load_samples(request)
    hop = max(32, rate//2000)
    count = len(samples)//hop
    if count<12: return result('digital-bursts', notes=['Capture too short for burst timing.'])
    powers = np.mean(np.abs(samples[:count*hop].reshape(count,hop))**2,axis=1)
    low,high = np.percentile(powers,[20,95])
    candidates=[]
    intervals=[]
    if high > max(1e-10,low*4):
        active = powers > max(low*4, high*.15)
        edges=np.diff(np.r_[False,active,False].astype(np.int8))
        starts=np.flatnonzero(edges==1);ends=np.flatnonzero(edges==-1)
        intervals=[dict(start_s=float(a*hop/rate),duration_s=float((b-a)*hop/rate)) for a,b in zip(starts,ends)][:200]
        if len(intervals)>=3:
            candidates=[dict(label='Burst/packet-like activity',confidence='waveform-candidate',evidence=f'{len(starts)} distinct energy bursts',
                             median_duration_ms=float(np.median(ends-starts)*hop/rate*1000))]
    return result('digital-bursts',candidates=candidates,bursts=intervals,
                  notes=['Energy bursts may be digital packets, keyed analog transmissions, interference or switching noise. No payload/protocol is inferred.'])

def ofdm(request):
    samples,rate=load_samples(request,2_000_000)
    if not np.iscomplexobj(samples) or rate<10_000_000:
        return [], ['OFDM short-training check needs complex IQ at >=10 MS/s; a 20 MHz Wi-Fi channel needs approximately 20 MS/s.']
    delay=round(rate*.8e-6);window=delay*8
    a=samples[:-delay];b=samples[delay:]
    if len(a)<window: return [],['Capture too short.']
    # Running normalized correlation at the legacy 802.11 short-training period.
    def running(v):
        sums=np.r_[0,np.cumsum(v)];return sums[window:]-sums[:-window]
    numer=np.abs(running(a*np.conj(b)))
    denom=np.sqrt(running(abs(a)**2)*running(abs(b)**2))
    score=numer/np.maximum(denom,1e-12)
    # Pure CW also repeats at any delay: reject near-constant instantaneous phase.
    phase=np.angle(samples[1:]*samples[:-1].conj())
    if len(score) and np.max(score)>.8 and np.std(phase)>.2:
        return [dict(label='802.11-like repeated training candidate',confidence='waveform-candidate',evidence='0.8 us normalized IQ repetition',correlation=float(np.max(score)))], ['OFDM repetition is not a decoded Wi-Fi frame. Confirm using a valid frame CRC or a monitor-mode PCAP.']
    return [],['No strong 802.11 short-training candidate found in the inspected window.']

def walkie_tones(request):
    samples,rate=channel(request,8000)
    if len(samples)<rate//2:return []
    freqs,power=signal.welch(samples,rate,nperseg=min(len(samples),16000))
    mask=(freqs>=65)&(freqs<=255)
    values=power[mask]
    if len(values)==0:return []
    idx=np.argmax(values)
    if values[idx]>max(1e-12,np.median(values)*30):
        return [dict(label='Low-frequency squelch-tone candidate',tone_hz=float(freqs[mask][idx]),confidence='waveform-candidate',evidence='narrow audio peak in CTCSS range')]
    return []

def identify(name,request):
    if name=='chirp-id':return chirps(request)
    if name=='digital-bursts':return bursts(request)
    candidates=band_candidates(name,request)
    notes=['Offline allocation context, not a live registry lookup or proof of protocol. Verify national allocations and channel bandwidth.']
    if name=='wifi-id':
        c,n=ofdm(request);candidates+=c;notes+=n
    if name=='walkie-id': candidates+=walkie_tones(request)
    if name=='encryption-id':
        notes=['Encryption cannot be established from noise, entropy or an unknown modulation. Supply a decoded PCAP with explicit protection/encryption fields; no encryption claim is made from raw IQ/audio.']
    if name=='hid-id': notes+=['Raw IQ cannot establish mouse/keyboard identity. A BLE HID UUID, Bluetooth device class, or a known proprietary protocol is required.']
    if name=='aviation-atis-id': notes+=['Aviation ATIS is spoken information; there is no universal ATIS packet header. Carrier/channel context does not establish station identity.']
    return result(name,candidates=candidates,notes=notes)

def chirps(request):
    samples,rate=load_samples(request,2000000)
    phase=np.angle(samples[1:]*samples[:-1].conj())
    size=max(64,rate//2000);count=len(phase)//size
    slopes=[]
    for block in phase[:count*size].reshape(count,size):
        derivative=np.diff(block);median=float(np.median(derivative))
        if abs(median)>1e-6 and np.median(abs(derivative-median))<abs(median)*.15:
            slopes.append(median*rate*rate/(2*np.pi))
    candidates=[]
    if len(slopes)>=4:
        candidates=[dict(label='Repeated linear chirp candidate',confidence='waveform-candidate',evidence='consistent instantaneous-frequency slope in multiple windows',windows=len(slopes),median_slope_hz_per_s=float(np.median(slopes)))]
    return result('chirp-id',candidates=candidates,notes=['Chirps may be LoRa, radar, sounders or other waveforms. This does not decode LoRa or establish network identity.'])
