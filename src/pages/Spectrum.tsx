import { Activity, Play, Square } from "lucide-react";
import ReactECharts from "echarts-for-react";
import { useBeaconCapture } from "../hooks/useBeaconCapture";
import { useMemo, useState } from "react";

export function Spectrum() {
    const { beacons, isCapturing, startCapture, stopCapture } = useBeaconCapture();
    const [band, setBand] = useState<'2.4' | '5' | '6'>('2.4');

    const handleToggleCapture = () => {
        if (isCapturing) stopCapture();
        else startCapture('wlan0'); // adjust as needed
    };

    // Prepare ECharts Options perfectly memoized
    const chartOptions = useMemo(() => {
        const beaconList = Array.from(beacons.values());

        let minX = 0, maxX = 15;
        if (band === '5') { minX = 30; maxX = 170; }
        else if (band === '6') { minX = 0; maxX = 240; }

        const series = beaconList.map((b) => {
            let ch = b.channel;
            // Basic band filtering based on channel number ranges
            if (band === '2.4' && ch > 14) return null;
            if (band === '5' && (ch <= 14 || ch > 170)) return null;
            if (band === '6' && ch < 1) return null; // Approximation

            // Width of curve: ~2 for 2.4GHz, varying for others
            const width = band === '2.4' ? 2 : 4;

            return {
                name: b.ssid || b.bssid,
                type: 'line',
                smooth: true,
                showSymbol: false,
                lineStyle: { width: 2 },
                areaStyle: { opacity: 0.1 },
                data: [
                    [ch - width, -100],
                    [ch - width / 2, b.rssi - 15],
                    [ch, b.rssi],
                    [ch + width / 2, b.rssi - 15],
                    [ch + width, -100]
                ]
            };
        }).filter(Boolean);

        return {
            backgroundColor: 'transparent',
            tooltip: {
                trigger: 'item',
                formatter: (params: any) => {
                    return `<b>${params.seriesName}</b><br/>Channel: ${params.value[0]}<br/>RSSI: ${params.value[1]} dBm`;
                }
            },
            xAxis: {
                type: 'value',
                min: minX,
                max: maxX,
                name: 'Channel',
                nameLocation: 'middle',
                nameGap: 25,
                axisLine: { lineStyle: { color: '#444' } },
                axisLabel: { color: '#888' },
                splitLine: { show: false }
            },
            yAxis: {
                type: 'value',
                min: -100,
                max: -20,
                name: 'RSSI (dBm)',
                nameLocation: 'middle',
                nameGap: 35,
                axisLine: { lineStyle: { color: '#444' } },
                axisLabel: { color: '#888' },
                splitLine: { lineStyle: { color: '#222', type: 'dashed' } }
            },
            series: series,
            animation: false // Disable default animation for fluid updates
        };
    }, [beacons, band]);

    return (
        <div className="h-full flex flex-col animate-in fade-in slide-in-from-bottom-4 duration-500">
            <header className="mb-8 flex justify-between items-start">
                <div>
                    <h1 className="text-3xl font-mono font-bold tracking-tight text-foreground flex items-center gap-3">
                        <span className="text-glow">SPECTRUM</span> <span className="opacity-50 text-muted-foreground font-sans text-2xl font-normal">// 2.4GHz & 5GHz</span>
                    </h1>
                    <p className="text-muted-foreground mt-2 font-mono text-sm uppercase tracking-wider">Real-time Topology Analysis</p>
                </div>
                <button
                    onClick={handleToggleCapture}
                    className={`flex items-center gap-2 px-4 py-2 rounded font-mono text-sm uppercase tracking-widest border transition-all ${isCapturing
                        ? "bg-destructive/10 border-destructive text-destructive hover:bg-destructive hover:text-white"
                        : "bg-primary/10 border-primary text-primary hover:bg-primary hover:text-black"
                        }`}
                >
                    {isCapturing ? <><Square className="w-4 h-4" /> Stop Capture</> : <><Play className="w-4 h-4" /> Start Capture</>}
                </button>
            </header>

            <div className="flex-1 glass-panel rounded-xl border border-border/40 p-6 flex flex-col">
                <div className="flex items-center justify-between mb-4 border-b border-border/40 pb-4">
                    <div className="flex gap-4">
                        <button
                            onClick={() => setBand('2.4')}
                            className={`px-4 py-1 text-xs font-mono tracking-widest uppercase border rounded transition-colors ${band === '2.4' ? 'border-primary text-primary bg-primary/10' : 'border-border text-muted-foreground hover:border-primary/50'}`}
                        >
                            2.4 GHz
                        </button>
                        <button
                            onClick={() => setBand('5')}
                            className={`px-4 py-1 text-xs font-mono tracking-widest uppercase border rounded transition-colors ${band === '5' ? 'border-primary text-primary bg-primary/10' : 'border-border text-muted-foreground hover:border-primary/50'}`}
                        >
                            5 GHz
                        </button>
                        <button
                            onClick={() => setBand('6')}
                            className={`px-4 py-1 text-xs font-mono tracking-widest uppercase border rounded transition-colors ${band === '6' ? 'border-primary text-primary bg-primary/10' : 'border-border text-muted-foreground hover:border-primary/50'}`}
                        >
                            6 GHz
                        </button>
                    </div>
                    <div className={`flex items-center gap-2 font-mono text-xs uppercase tracking-wider ${isCapturing ? 'text-primary' : 'text-muted-foreground'}`}>
                        <Activity className={`w-4 h-4 ${isCapturing ? 'animate-pulse' : ''}`} />
                        {isCapturing ? 'Scanning' : 'Offline'}
                    </div>
                </div>
                <div className="flex-1 border border-border/20 rounded-lg bg-black/20 flex items-center justify-center p-2">
                    <ReactECharts
                        option={chartOptions}
                        style={{ height: '100%', width: '100%' }}
                        notMerge={false}
                        lazyUpdate={true}
                    />
                </div>
            </div>
        </div>
    );
}
