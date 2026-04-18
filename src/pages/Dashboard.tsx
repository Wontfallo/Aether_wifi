import { Wifi, Play, Square, ArrowUpDown } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useBeaconCapture } from "../hooks/useBeaconCapture";

export function Dashboard() {
    const { beacons, isCapturing, startCapture, stopCapture, error } = useBeaconCapture();
    const [sortOrder, setSortOrder] = useState<'asc' | 'desc'>('desc');
    const [monitorIface, setMonitorIface] = useState('wlan0');
    const autoStartAttemptedRef = useRef(false);

    useEffect(() => {
        invoke<string>('get_monitor_interface').then((iface) => {
            setMonitorIface(iface);
        }).catch(() => {});
    }, []);

    useEffect(() => {
        if (autoStartAttemptedRef.current || isCapturing || error) {
            return;
        }

        autoStartAttemptedRef.current = true;
        void startCapture(monitorIface);
    }, [error, isCapturing, monitorIface, startCapture]);

    const handleToggleCapture = () => {
        if (isCapturing) {
            stopCapture();
        } else {
            startCapture(monitorIface);
        }
    };

    const sortedNetworks = useMemo(() => {
        const list = Array.from(beacons.values());
        return list.sort((a, b) => {
            if (sortOrder === 'asc') return a.rssi - b.rssi;
            return b.rssi - a.rssi;
        });
    }, [beacons, sortOrder]);

    const toggleSort = () => {
        setSortOrder(prev => prev === 'desc' ? 'asc' : 'desc');
    };

    return (
        <div className="h-full flex flex-col animate-in fade-in slide-in-from-bottom-4 duration-500">
            <header className="mb-8 flex justify-between items-start">
                <div>
                    <h1 className="text-3xl font-mono font-bold tracking-tight text-foreground flex items-center gap-3">
                        <Wifi className="w-8 h-8 text-primary" />
                        <span className="text-glow">AETHER</span> <span className="opacity-50 text-muted-foreground font-sans text-2xl font-normal">// DASHBOARD</span>
                    </h1>
                    <p className="text-muted-foreground mt-2 font-mono text-sm uppercase tracking-wider">Active Monitoring: Interface {monitorIface} (Monitor Mode)</p>
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

            {error && (
                <div className="mb-6 p-4 border border-destructive/50 bg-destructive/10 text-destructive rounded-lg font-mono text-sm">
                    {error}
                </div>
            )}

            <div className="grid grid-cols-1 md:grid-cols-3 gap-6 mb-6">
                {/* Stat Cards */}
                <div className="glass-panel p-6 rounded-xl relative overflow-hidden group">
                    <div className="absolute top-0 left-0 w-1 h-full bg-border group-hover:bg-primary transition-colors"></div>
                    <p className="text-muted-foreground font-mono text-xs uppercase tracking-widest mb-2">Visible Networks</p>
                    <h2 className="text-4xl font-bold font-mono text-primary">{beacons.size}</h2>
                </div>
                <div className="glass-panel p-6 rounded-xl relative overflow-hidden group">
                    <div className="absolute top-0 left-0 w-1 h-full bg-border group-hover:bg-destructive transition-colors"></div>
                    <p className="text-muted-foreground font-mono text-xs uppercase tracking-widest mb-2">Channel Congestion</p>
                    <h2 className="text-4xl font-bold font-mono text-destructive">
                        {beacons.size > 20 ? "High" : beacons.size > 10 ? "Med" : "Low"}
                    </h2>
                </div>
                <div className="glass-panel p-6 rounded-xl relative overflow-hidden group">
                    <div className="absolute top-0 left-0 w-1 h-full bg-border group-hover:bg-radar-green transition-colors"></div>
                    <p className="text-muted-foreground font-mono text-xs uppercase tracking-widest mb-2">Unique BSSIDs</p>
                    <h2 className="text-4xl font-bold font-mono text-radar-green">{beacons.size}</h2>
                </div>
            </div>

            {/* Main Data Table */}
            <div className="flex-1 glass-panel rounded-xl overflow-hidden flex flex-col border border-border/40">
                <div className="px-6 py-4 border-b border-border/40 bg-muted/20 flex justify-between items-center">
                    <h3 className="font-mono text-sm text-muted-foreground uppercase tracking-widest">Network Intercept Data</h3>
                    <div className="flex gap-2 items-center">
                        <div className={`w-2 h-2 rounded-full ${isCapturing ? 'bg-radar-green animate-pulse-fast' : 'bg-muted-foreground'}`}></div>
                        <span className={`font-mono text-[10px] uppercase tracking-wider ${isCapturing ? 'text-radar-green' : 'text-muted-foreground'}`}>
                            {isCapturing ? 'Live Feed' : 'Offline'}
                        </span>
                    </div>
                </div>
                <div className="flex-1 overflow-auto bg-black/20">
                    <table className="w-full text-left font-mono text-sm border-collapse">
                        <thead className="sticky top-0 bg-background/95 backdrop-blur z-10 border-b border-border/40">
                            <tr>
                                <th className="py-3 px-6 font-normal text-muted-foreground uppercase tracking-widest">BSSID</th>
                                <th className="py-3 px-6 font-normal text-muted-foreground uppercase tracking-widest">SSID</th>
                                <th className="py-3 px-6 font-normal text-muted-foreground uppercase tracking-widest text-center">CH</th>
                                <th
                                    className="py-3 px-6 font-normal text-muted-foreground uppercase tracking-widest cursor-pointer hover:text-primary transition-colors flex items-center gap-2 justify-end"
                                    onClick={toggleSort}
                                >
                                    RSSI (dBm) <ArrowUpDown className="w-3 h-3" />
                                </th>
                            </tr>
                        </thead>
                        <tbody>
                            {sortedNetworks.length === 0 ? (
                                <tr>
                                    <td colSpan={4} className="py-8 text-center text-muted-foreground italic">
                                        {isCapturing ? "Listening for beacons..." : "Start capture to intercept networks."}
                                    </td>
                                </tr>
                            ) : (
                                sortedNetworks.map((net) => (
                                    <tr key={net.bssid} className="border-b border-border/10 hover:bg-white/5 transition-colors group">
                                        <td className="py-3 px-6 text-foreground/80 group-hover:text-primary transition-colors">{net.bssid}</td>
                                        <td className="py-3 px-6">{net.ssid || <span className="opacity-50 italic">&lt;hidden&gt;</span>}</td>
                                        <td className="py-3 px-6 text-center text-muted-foreground">{net.channel}</td>
                                        <td className="py-3 px-6 text-right">
                                            <div className="flex items-center justify-end gap-3">
                                                <div className="w-16 h-1.5 bg-black rounded-full overflow-hidden">
                                                    <div
                                                        className={`h-full ${net.rssi > -60 ? 'bg-radar-green' : net.rssi > -80 ? 'bg-primary' : 'bg-destructive'}`}
                                                        style={{ width: `${Math.max(0, Math.min(100, (net.rssi + 100) * 1.5))}%` }}
                                                    />
                                                </div>
                                                <span className={`${net.rssi > -60 ? 'text-radar-green' : net.rssi > -80 ? 'text-primary' : 'text-destructive'} font-bold`}>
                                                    {net.rssi}
                                                </span>
                                            </div>
                                        </td>
                                    </tr>
                                ))
                            )}
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
    );
}
