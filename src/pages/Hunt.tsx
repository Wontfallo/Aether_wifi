import { Crosshair, Radio, Square } from "lucide-react";
import ReactECharts from "echarts-for-react";
import { useMemo, useState } from "react";
import { useBeaconCapture } from "../hooks/useBeaconCapture";

export function Hunt() {
    const [targetMac, setTargetMac] = useState("");
    const [channelLock, setChannelLock] = useState("Auto-Scan");
    const { beaconStream, isCapturing, startCapture, stopCapture } = useBeaconCapture(5000);

    const handleToggleCapture = () => {
        if (isCapturing) stopCapture();
        else startCapture('wlan0'); // default capturing interface
    };

    const cleanMac = targetMac.trim().toLowerCase();

    // Data specifically for the target MAC
    const targetData = useMemo(() => {
        if (!cleanMac) return [];
        return beaconStream.filter(b => b.bssid.toLowerCase() === cleanMac);
    }, [beaconStream, cleanMac]);

    const latestRssi = targetData.length > 0 ? targetData[targetData.length - 1].rssi : null;

    const chartOptions = useMemo(() => {
        const now = Date.now();
        const sixtySecondsAgo = now - 60000;

        // Map data to x,y format, only keeping last 60 seconds
        const seriesData = targetData
            .filter(b => b.timestamp_ms >= sixtySecondsAgo)
            .map(b => [b.timestamp_ms, b.rssi]);

        return {
            backgroundColor: 'transparent',
            tooltip: { trigger: 'axis' },
            xAxis: {
                type: 'time',
                min: sixtySecondsAgo,
                max: now,
                splitLine: { show: false },
                axisLine: { lineStyle: { color: '#444' } },
                axisLabel: { color: '#888', formatter: '{mm}:{ss}' }
            },
            yAxis: {
                type: 'value',
                min: -100,
                max: -20,
                name: 'RSSI (dBm)',
                splitLine: { lineStyle: { color: '#ef4444', opacity: 0.1 } },
                axisLine: { lineStyle: { color: '#444' } },
                axisLabel: { color: '#ef4444' }
            },
            series: [{
                name: 'Target RSSI',
                type: 'line',
                smooth: true,
                showSymbol: false,
                lineStyle: { width: 3, color: '#ef4444' },
                areaStyle: {
                    color: {
                        type: 'linear', x: 0, y: 0, x2: 0, y2: 1,
                        colorStops: [{ offset: 0, color: 'rgba(239, 68, 68, 0.5)' }, { offset: 1, color: 'rgba(239, 68, 68, 0)' }]
                    }
                },
                data: seriesData
            }],
            animation: false
        };
    }, [targetData]);

    return (
        <div className="h-full flex flex-col animate-in fade-in slide-in-from-bottom-4 duration-500">
            <header className="mb-8 flex justify-between items-start">
                <div>
                    <h1 className="text-3xl font-mono font-bold tracking-tight text-foreground flex items-center gap-3">
                        <Crosshair className="w-8 h-8 text-destructive" />
                        <span className="text-glow text-destructive">HUNT</span> <span className="opacity-50 text-muted-foreground font-sans text-2xl font-normal">// TARGETED TRACKING</span>
                    </h1>
                    <p className="text-muted-foreground mt-2 font-mono text-sm uppercase tracking-wider">RSSI Tracker & Direction Finding</p>
                </div>
            </header>

            <div className="grid grid-cols-1 lg:grid-cols-4 gap-6 h-full">
                {/* Sidebar Settings / Target Selector */}
                <div className="lg:col-span-1 glass-panel rounded-xl p-6 border border-border/40 flex flex-col gap-6">
                    <div>
                        <label className="text-xs font-mono uppercase tracking-widest text-muted-foreground mb-2 block">Target MAC</label>
                        <input
                            type="text"
                            placeholder="00:11:22:33:44:55"
                            value={targetMac}
                            onChange={(e) => setTargetMac(e.target.value)}
                            className="w-full bg-black/40 border border-border/60 rounded px-4 py-2 font-mono text-sm text-primary focus:border-primary focus:outline-none transition-colors uppercase"
                        />
                    </div>
                    <div>
                        <label className="text-xs font-mono uppercase tracking-widest text-muted-foreground mb-2 block">Lock Channel</label>
                        <select
                            value={channelLock}
                            onChange={(e) => setChannelLock(e.target.value)}
                            className="w-full bg-black/40 border border-border/60 rounded px-4 py-2 font-mono text-sm text-foreground focus:border-primary focus:outline-none transition-colors appearance-none"
                        >
                            <option>Auto-Scan</option>
                            <option>CH 1 (2.4GHz)</option>
                            <option>CH 6 (2.4GHz)</option>
                            <option>CH 11 (2.4GHz)</option>
                        </select>
                    </div>
                    <button
                        onClick={handleToggleCapture}
                        className={`mt-auto w-full transition-all rounded py-3 font-mono text-sm uppercase tracking-widest flex items-center justify-center gap-2 ${isCapturing
                            ? "bg-destructive border border-destructive text-destructive-foreground hover:bg-destructive/80"
                            : "bg-destructive/10 border border-destructive/50 text-destructive hover:bg-destructive hover:text-destructive-foreground"
                            }`}
                    >
                        {isCapturing ? <><Square className="w-4 h-4" /> Disengage</> : <><Radio className="w-4 h-4" /> Engage Tracker</>}
                    </button>
                </div>

                {/* Main Radar Graph */}
                <div className="lg:col-span-3 glass-panel rounded-xl border border-destructive/30 p-6 flex flex-col relative overflow-hidden">
                    {/* Background Scanner Effect */}
                    <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(239,68,68,0.05)_0%,transparent_70%)] pointer-events-none"></div>

                    <div className="flex justify-between items-center mb-6 z-10">
                        <h3 className="font-mono text-sm uppercase tracking-widest text-destructive">Signal Strength (dBm)</h3>
                        <span className="font-mono text-3xl font-bold text-destructive">
                            {latestRssi !== null ? latestRssi : "- --"}
                        </span>
                    </div>

                    <div className="flex-1 bg-black/40 border border-border/20 rounded relative z-10 p-2">
                        {!cleanMac ? (
                            <div className="absolute inset-0 flex flex-col items-center justify-center">
                                <div className="w-32 h-32 rounded-full border border-destructive/20 flex items-center justify-center relative overflow-hidden mb-4">
                                    <div className="absolute w-full h-[1px] bg-destructive/40 top-1/2 -translate-y-1/2 animate-pulse-fast pointer-events-none"></div>
                                    <div className="absolute h-full w-[1px] bg-destructive/40 left-1/2 -translate-x-1/2 animate-pulse-fast pointer-events-none"></div>
                                </div>
                                <span className="font-mono text-sm text-destructive/50 uppercase tracking-widest">No Target MAC Specified</span>
                            </div>
                        ) : (
                            <ReactECharts
                                option={chartOptions}
                                style={{ height: '100%', width: '100%' }}
                                notMerge={true}
                                lazyUpdate={true}
                            />
                        )}
                    </div>
                </div>
            </div>
        </div>
    );
}
