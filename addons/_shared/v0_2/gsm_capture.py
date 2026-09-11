#!/usr/bin/python3
"""Offline BCCH flowgraph; retain actual GSMTAP PDUs without a network interface."""
import json,sys,threading
from gnuradio import blocks,gr,gsm
from gnuradio.gsm import arfcn
import pmt
from pcapio import export,udp_ip

class Collector(gr.basic_block):
    def __init__(self):
        gr.basic_block.__init__(self,name='THUGSRF BCCH packet collector',in_sig=None,out_sig=None)
        self.packets=[];self.lock=threading.Lock()
        self.message_port_register_in(pmt.intern('in'));self.set_msg_handler(pmt.intern('in'),self.receive)
    def receive(self,message):
        payload=bytes(pmt.to_python(pmt.cdr(message)))
        # GSMTAP v2, >=16-byte header, BCCH only; leave channel headers/payload intact.
        if len(payload)<16 or payload[0]!=2 or payload[1]*4<16 or payload[12]&0x7f!=1:return
        frame=int.from_bytes(payload[8:12],'big')
        with self.lock:
            if len(self.packets)<10000:self.packets.append((frame*60/13000,udp_ip(payload)))

def main():
    spec=json.loads(sys.argv[1]);frequency=spec['frequency'];channel=arfcn.downlink2arfcn(frequency)
    if channel is None:raise ValueError('frequency does not map to a supported GSM downlink ARFCN')
    graph=gr.top_block('THUGSRF offline BCCH PCAP')
    source=blocks.file_source(gr.sizeof_gr_complex,spec['path'],False)
    adapter=gsm.gsm_input(ppm=0,osr=4,fc=frequency,samp_rate_in=1000000)
    receiver=gsm.receiver(4,[channel],[]);offset=gsm.clock_offset_control(frequency,1000000)
    dummy=gsm.dummy_burst_filter();slots=gsm.burst_timeslot_filter(0)
    mapper=gsm.gsm_bcch_ccch_demapper(0);decoder=gsm.control_channels_decoder();sink=Collector()
    graph.connect(source,adapter,receiver)
    graph.msg_connect(receiver,'measurements',offset,'measurements');graph.msg_connect(offset,'ctrl',adapter,'ctrl_in')
    graph.msg_connect(receiver,'C0',dummy,'in');graph.msg_connect(dummy,'out',slots,'in')
    graph.msg_connect(slots,'out',mapper,'bursts');graph.msg_connect(mapper,'bursts',decoder,'bursts');graph.msg_connect(decoder,'msgs',sink,'in')
    graph.run()
    # GSM frame number wraps after a hyperframe; preserve it in GSMTAP, use relative ordering in PCAP.
    packets=sink.packets
    if packets:
        start=packets[0][0];wrap=0.;last=start;relative=[]
        for stamp,payload in packets:
            if stamp+wrap<last:wrap+=2715648*60/13000
            last=stamp+wrap;relative.append((last-start,payload))
        packets=relative
    result=export(spec['request'],101,packets,'GSM BCCH / GSMTAP','Relative GSM frame-number timing, first decoded BCCH frame at zero; IPv4/UDP envelope is generated offline and was never transmitted')
    print('THUGSRF_RESULT='+json.dumps(result))
if __name__=='__main__':main()
