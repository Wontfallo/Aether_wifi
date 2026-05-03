import { Activity, Play, Square, Layers, Waves, Columns } from "lucide-react";
import ReactECharts from "echarts-for-react";
import { useBeaconCapture } from "../hooks/useBeaconCapture";
import { useMemo, useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";

// Distance formula (approximate)
const estimateDistance = (rssi: number, freqMhz: number) => {
    // Free Space Path Loss approximation
    const fspl = 27.55 - (20 * Math.log10(freqMhz || 2412)) + Math.abs(rssi);
    return Math.pow(10, fspl / 20).toFixed(1);
};

// Vibrant palette for curves
const palette = ['#00E5FF', '#FF00E5', '#E5FF00', '#00FF4D', '#FF4D00', '#7A00FF', '#FF0055'];
const getColor = (bssid: string) => {
    const sum = bssid.split(':').reduce((a, b) => a + parseInt(b, 16), 0);
    return palette[sum % palette.length];
};

export function Spectrum() {
    const { beacons, isCapturing, startCapture, stopCapture } = useBeaconCapture();
    const [band, setBand] = useState<'2.4' | '5' | '6'>('2.4');
    const [viewMode, setViewMode] = useState<'split' | 'spectrum' | 'waterfall'>('split');
    
    // Waterfall History State
    const [history, setHistory] = useState<{ time: string, data: { ch: number, rssi: number, bssid: string }[] }[]>([]);

    useEffect(() => {
        if (!isCapturing) return;
        const interval = setInterval(() => {
            const time = new Date().toLocaleTimeString('en-US', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' });
            const snapshot = Array.from(beacons.values()).map(b => ({
                ch: b.channel,
                rssi: b.rssi,
                bssid: b.bssid
            }));
            setHistory(prev => {
                const next = [...prev, { time, data: snapshot }];
                // Keep 40 ticks for the waterfall to look robust without lagging
                if (next.length > 40) next.shift(); 
                return next;
            });
        }, 1000);
        return () => clearInterval(interval);
    }, [isCapturing, beacons]);

    const handleToggleCapture = () => {
        if (isCapturing) stopCapture();
        else startCapture('wlan0');
    };

    // Parabola (Spectrum) Options
    const spectrumOptions = useMemo(() => {
        const beaconList = Array.from(beacons.values());
        let minX = 0, maxX = 15;
        if (band === '5') { minX = 30; maxX = 175; }
        else if (band === '6') { minX = 0; maxX = 240; }

        const series = beaconList.map((b) => {
            const ch = b.channel;
            // Filter bands
            if (band === '2.4' && ch > 14) return null;
            if (band === '5' && (ch <= 14 || ch > 175)) return null;
            if (band === '6' && ch < 1) return null;

            const width = band === '2.4' ? 2.5 : 4;
            const color = getColor(b.bssid);
            const dist = estimateDistance(b.rssi, b.frequency_mhz);

            return {
                name: `${b.ssid || '<hidden>'} (${b.bssid})`,
                type: 'line',
                smooth: true,
                showSymbol: false,
                lineStyle: { width: 3, color },
                areaStyle: {
                    opacity: 0.15,
                    color: {
                        type: 'linear', x: 0, y: 0, x2: 0, y2: 1,
                        colorStops: [{ offset: 0, color }, { offset: 1, color: 'rgba(0,0,0,0)' }]
                    }
                },
                data: [
                    [ch - width, -100],
                    [ch - width / 2, b.rssi - 15],
                    // Attach metadata to the peak for the tooltip and show a label/symbol
                    { 
                        value: [ch, b.rssi, { bssid: b.bssid, dist, freq: b.frequency_mhz, rssi: b.rssi, vendor: b.vendor }], 
                        symbol: 'circle', 
                        symbolSize: 8, 
                        itemStyle: { color: '#fff' }, 
                        label: { show: true, formatter: b.ssid || b.bssid, position: 'top', color: '#fff', fontSize: 10, fontFamily: 'monospace' } 
                    },
                    [ch + width / 2, b.rssi - 15],
                    [ch + width, -100]
                ]
            };
        }).filter(Boolean);

        return {
            backgroundColor: 'transparent',
            tooltip: {
                trigger: 'item',
                backgroundColor: 'rgba(10, 10, 15, 0.95)',
                borderColor: '#444',
                padding: 12,
                textStyle: { color: '#fff', fontFamily: 'monospace', fontSize: 12 },
                formatter: (p: any) => {
                    const meta = p.data[2]; 
                    if (!meta) return '';
                    return `
                        <div style="font-weight:bold; border-bottom:1px solid #333; padding-bottom:8px; margin-bottom:8px; color:${p.color}; letter-spacing:1px;">
                            ${p.seriesName}
                        </div>
                        <table style="width:100%; font-size:11px; opacity:0.9;">
                            <tr><td style="color:#888; padding-right:10px;">VENDOR</td><td style="text-align:right;">${meta.vendor || 'Unknown'}</td></tr>
                            <tr><td style="color:#888; padding-right:10px;">CHANNEL</td><td style="text-align:right;">${p.value[0]} (${meta.freq} MHz)</td></tr>
                            <tr><td style="color:#888; padding-right:10px;">POWER</td><td style="text-align:right; color:${meta.rssi > -60 ? '#00FF4D' : meta.rssi > -80 ? '#E5FF00' : '#FF4D00'}; font-weight:bold;">${meta.rssi} dBm</td></tr>
                            <tr><td style="color:#888; padding-right:10px;">EST. DIST</td><td style="text-align:right;">~${meta.dist} m</td></tr>
                        </table>
                    `;
                }
            },
            grid: { top: 20, bottom: 60, left: 50, right: 20 },
            dataZoom: [
                { type: 'inside', xAxisIndex: 0, filterMode: 'filter' },
                { type: 'slider', xAxisIndex: 0, textStyle: { color: '#888' }, borderColor: 'transparent', fillerColor: 'rgba(255,255,255,0.1)' }
            ],
            xAxis: {
                type: 'value', min: minX, max: maxX, name: 'Channel', nameLocation: 'middle', nameGap: 25,
                axisLine: { lineStyle: { color: '#666' } },
                axisLabel: { color: '#aaa', fontFamily: 'monospace' },
                splitLine: { show: false }
            },
            yAxis: {
                type: 'value', min: -100, max: -20, name: 'RSSI', nameLocation: 'middle', nameGap: 35,
                axisLine: { lineStyle: { color: '#666' } },
                axisLabel: { color: '#aaa', fontFamily: 'monospace' },
                splitLine: { lineStyle: { color: '#222', type: 'dashed' } }
            },
            series: series,
            animation: false // Disable default animation for fluid real-time updates
        };
    }, [beacons, band]);

    // Waterfall Options
    const waterfallOptions = useMemo(() => {
        let channels: string[] = [];
        if (band === '2.4') channels = Array.from({length: 14}, (_, i) => String(i + 1));
        if (band === '5') channels = Array.from({length: 36}, (_, i) => String(36 + i * 4));
        if (band === '6') channels = Array.from({length: 59}, (_, i) => String(1 + i * 4));

        const data: [number, number, number][] = [];
        const times = history.map(h => h.time);
        
        history.forEach((h, timeIdx) => {
            h.data.forEach(d => {
                const chStr = String(d.ch);
                const xIdx = channels.indexOf(chStr);
                if (xIdx !== -1) {
                    data.push([xIdx, timeIdx, d.rssi]);
                }
            });
        });

        return {
            backgroundColor: 'transparent',
            tooltip: { 
                position: 'top',
                backgroundColor: 'rgba(10, 10, 15, 0.95)',
                borderColor: '#444',
                textStyle: { color: '#fff', fontFamily: 'monospace', fontSize: 11 },
                formatter: (p: any) => `Time: ${times[p.value[1]]}<br/>Channel: ${channels[p.value[0]]}<br/>RSSI: ${p.value[2]} dBm` 
            },
            grid: { top: 10, bottom: 20, left: 50, right: 20 },
            xAxis: {
                type: 'category', data: channels,
                axisLine: { show: false }, axisLabel: { color: '#aaa', fontFamily: 'monospace' }, splitLine: { show: false }
            },
            yAxis: {
                type: 'category', data: times,
                axisLine: { show: false }, 
                axisLabel: { color: '#555', fontFamily: 'monospace', formatter: (v: string) => v.split(':')[2] }, 
                splitLine: { show: false }
            },
            visualMap: {
                min: -100, max: -30,
                calculable: true,
                orient: 'vertical',
                show: false, // Hide the legend to save space
                inRange: { color: ['#000000', '#0a0a4a', '#1e40af', '#00d4ff', '#00ff4d', '#e5ff00', '#ff0055'] }
            },
            series: [{
                name: 'Waterfall',
                type: 'heatmap',
                data: data,
                emphasis: { itemStyle: { shadowBlur: 10, shadowColor: 'rgba(0, 0, 0, 0.5)' } },
                progressive: 0,
                animation: false
            }]
        };
    }, [history, band]);

    return (
        <div className="h-full flex flex-col animate-in fade-in slide-in-from-bottom-4 duration-500">
            <header className="mb-6 flex justify-between items-start">
                <div>
                    <h1 className="text-3xl font-mono font-bold tracking-tight text-foreground flex items-center gap-3">
                        <Activity className="text-primary w-8 h-8" />
                        <span className="text-glow">SPECTRUM</span> <span className="opacity-50 text-muted-foreground font-sans text-2xl font-normal">// ANALYZER</span>
                    </h1>
                    <p className="text-muted-foreground mt-2 font-mono text-sm uppercase tracking-wider">Topology & Spectral Heatmap</p>
                </div>
                <button
                    onClick={handleToggleCapture}
                    className={`flex items-center gap-2 px-5 py-2.5 rounded font-mono text-sm font-bold uppercase tracking-widest border-2 transition-all shadow-lg ${isCapturing
                        ? "bg-destructive/10 border-destructive text-destructive hover:bg-destructive hover:text-white shadow-destructive/20"
                        : "bg-primary/10 border-primary text-primary hover:bg-primary hover:text-black shadow-primary/20"
                        }`}
                >
                    {isCapturing ? <><Square className="w-4 h-4" /> Stop Scan</> : <><Play className="w-4 h-4" /> Start Scan</>}
                </button>
            </header>

            <div className="flex-1 glass-panel rounded-xl border border-border/40 p-4 flex flex-col min-h-0">
                {/* Control Bar */}
                <div className="flex items-center justify-between mb-4 bg-black/40 p-2 rounded-lg border border-border/30">
                    <div className="flex gap-2">
                        {(['2.4', '5', '6'] as const).map(b => (
                            <button key={b} onClick={() => setBand(b)} className={`px-4 py-1.5 text-xs font-mono font-bold tracking-widest uppercase border rounded transition-colors ${band === b ? 'border-primary text-primary bg-primary/15 shadow-[0_0_10px_rgba(var(--primary),0.2)]' : 'border-transparent text-muted-foreground hover:bg-white/5'}`}>
                                {b} GHz
                            </button>
                        ))}
                    </div>

                    <div className="flex items-center gap-4">
                        <div className="flex bg-black/50 p-1 rounded border border-border/30">
                            <button onClick={() => setViewMode('spectrum')} className={`p-1.5 rounded transition-colors ${viewMode === 'spectrum' ? 'bg-white/10 text-primary' : 'text-muted-foreground hover:text-white'}`} title="Curve View">
                                <Activity className="w-4 h-4" />
                            </button>
                            <button onClick={() => setViewMode('waterfall')} className={`p-1.5 rounded transition-colors ${viewMode === 'waterfall' ? 'bg-white/10 text-primary' : 'text-muted-foreground hover:text-white'}`} title="Waterfall View">
                                <Waves className="w-4 h-4" />
                            </button>
                            <button onClick={() => setViewMode('split')} className={`p-1.5 rounded transition-colors ${viewMode === 'split' ? 'bg-white/10 text-primary' : 'text-muted-foreground hover:text-white'}`} title="Split View">
                                <Columns className="w-4 h-4" />
                            </button>
                        </div>
                        <div className={`flex items-center gap-2 font-mono text-xs font-bold uppercase tracking-wider ${isCapturing ? 'text-radar-green' : 'text-muted-foreground'}`}>
                            <div className={`w-2 h-2 rounded-full ${isCapturing ? 'bg-radar-green animate-pulse shadow-[0_0_8px_#00FF4D]' : 'bg-muted-foreground'}`} />
                            {isCapturing ? 'Scanning...' : 'Offline'}
                        </div>
                    </div>
                </div>

                {/* Charts Area */}
                <div className="flex-1 flex flex-col gap-4 min-h-0">
                    <AnimatePresence mode="popLayout">
                        {/* SPECTRUM CHART */}
                        {(viewMode === 'spectrum' || viewMode === 'split') && (
                            <motion.div 
                                key="spectrum"
                                initial={{ opacity: 0, scale: 0.95 }}
                                animate={{ opacity: 1, scale: 1 }}
                                exit={{ opacity: 0, scale: 0.95 }}
                                className={`border border-border/20 rounded-lg bg-black/30 p-2 relative ${viewMode === 'split' ? 'flex-1 min-h-0' : 'h-full'}`}
                            >
                                <div className="absolute top-3 left-4 font-mono text-[10px] text-muted-foreground uppercase tracking-widest z-10 flex items-center gap-2">
                                    <Layers className="w-3 h-3" /> Topology
                                </div>
                                <ReactECharts option={spectrumOptions} style={{ height: '100%', width: '100%' }} notMerge={false} lazyUpdate={true} />
                            </motion.div>
                        )}

                        {/* WATERFALL CHART */}
                        {(viewMode === 'waterfall' || viewMode === 'split') && (
                            <motion.div 
                                key="waterfall"
                                initial={{ opacity: 0, scale: 0.95 }}
                                animate={{ opacity: 1, scale: 1 }}
                                exit={{ opacity: 0, scale: 0.95 }}
                                className={`border border-border/20 rounded-lg bg-black/30 p-2 relative ${viewMode === 'split' ? 'flex-1 min-h-0' : 'h-full'}`}
                            >
                                <div className="absolute top-3 left-4 font-mono text-[10px] text-muted-foreground uppercase tracking-widest z-10 flex items-center gap-2">
                                    <Waves className="w-3 h-3" /> Waterfall Heatmap
                                </div>
                                <ReactECharts option={waterfallOptions} style={{ height: '100%', width: '100%' }} notMerge={false} lazyUpdate={true} />
                            </motion.div>
                        )}
                    </AnimatePresence>
                </div>
            </div>
        </div>
    );
}
