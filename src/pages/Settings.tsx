/**
 * Aether — Settings Page
 *
 * Provides interface management: list adapters, toggle monitor/managed mode,
 * and configure the active capture interface.
 */

import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
    Settings2, RefreshCw, Wifi, WifiOff, ToggleLeft, ToggleRight,
    ChevronRight, AlertTriangle, CheckCircle2, Radio
} from "lucide-react";
import type { NetworkInterface, InterfaceModeResult } from "../types/capture";

export function SettingsPage() {
    const [interfaces, setInterfaces] = useState<NetworkInterface[]>([]);
    const [loading, setLoading] = useState(false);
    const [toggleLoading, setToggleLoading] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [lastResult, setLastResult] = useState<InterfaceModeResult | null>(null);

    const fetchInterfaces = useCallback(async () => {
        setLoading(true);
        setError(null);
        try {
            const ifaces = await invoke<NetworkInterface[]>("list_interfaces");
            setInterfaces(ifaces);
        } catch (err: unknown) {
            const msg = typeof err === "string" ? err : JSON.stringify(err);
            setError(msg);
        } finally {
            setLoading(false);
        }
    }, []);

    useEffect(() => {
        fetchInterfaces();
    }, [fetchInterfaces]);

    const handleToggleMode = async (ifaceName: string) => {
        setToggleLoading(ifaceName);
        setError(null);
        setLastResult(null);

        try {
            const result = await invoke<InterfaceModeResult>("toggle_interface_mode", {
                interfaceName: ifaceName,
            });
            setLastResult(result);
            // Refresh the interface list to show updated mode
            await fetchInterfaces();
        } catch (err: unknown) {
            const msg = typeof err === "string" ? err : JSON.stringify(err);
            setError(msg);
        } finally {
            setToggleLoading(null);
        }
    };

    const wireless = interfaces.filter(i => i.is_wireless);
    const wired = interfaces.filter(i => !i.is_wireless);

    return (
        <div className="h-full flex flex-col animate-in fade-in slide-in-from-bottom-4 duration-500">
            <header className="mb-6 flex justify-between items-start">
                <div>
                    <h1 className="text-3xl font-mono font-bold tracking-tight text-foreground flex items-center gap-3">
                        <Settings2 className="w-8 h-8 text-primary" />
                        <span className="text-glow">SETTINGS</span>{" "}
                        <span className="opacity-50 text-muted-foreground font-sans text-2xl font-normal">// INTERFACES</span>
                    </h1>
                    <p className="text-muted-foreground mt-2 font-mono text-sm uppercase tracking-wider">
                        Adapter Management & Mode Control
                    </p>
                </div>
                <button
                    onClick={fetchInterfaces}
                    disabled={loading}
                    className="flex items-center gap-2 px-4 py-2 rounded font-mono text-sm uppercase tracking-widest border border-primary/50 text-primary bg-primary/10 hover:bg-primary hover:text-black transition-all disabled:opacity-30"
                >
                    <RefreshCw className={`w-4 h-4 ${loading ? "animate-spin" : ""}`} />
                    Refresh
                </button>
            </header>

            {/* Result Banner */}
            {lastResult && (
                <div className={`mb-4 p-3 rounded-lg font-mono text-xs flex items-center gap-2 border ${lastResult.success ? "border-radar-green/50 bg-radar-green/10 text-radar-green" : "border-destructive/50 bg-destructive/10 text-destructive"}`}>
                    {lastResult.success ? <CheckCircle2 className="w-4 h-4 shrink-0" /> : <AlertTriangle className="w-4 h-4 shrink-0" />}
                    {lastResult.message}
                </div>
            )}

            {error && (
                <div className="mb-4 p-3 border border-destructive/50 bg-destructive/10 text-destructive rounded-lg font-mono text-xs flex items-center gap-2">
                    <AlertTriangle className="w-4 h-4 shrink-0" />
                    {error}
                </div>
            )}

            <div className="flex-1 overflow-auto space-y-6">
                {/* Wireless Interfaces */}
                <section>
                    <h2 className="font-mono text-sm uppercase tracking-widest text-primary mb-3 flex items-center gap-2">
                        <Wifi className="w-4 h-4" />
                        Wireless Adapters ({wireless.length})
                    </h2>
                    {wireless.length === 0 ? (
                        <div className="glass-panel rounded-xl border border-border/40 p-8 text-center">
                            <WifiOff className="w-10 h-10 text-muted-foreground/30 mx-auto mb-3" />
                            <p className="font-mono text-xs text-muted-foreground uppercase tracking-widest">No wireless adapters detected</p>
                            <p className="font-mono text-[10px] text-muted-foreground/60 mt-1">Check the Environment Doctor for setup instructions.</p>
                        </div>
                    ) : (
                        <div className="space-y-2">
                            {wireless.map((iface) => (
                                <div key={iface.name} className="glass-panel rounded-xl border border-border/40 p-4 flex items-center gap-4 group hover:border-primary/30 transition-colors">
                                    <div className={`w-10 h-10 rounded-lg flex items-center justify-center ${iface.mode === "monitor" ? "bg-radar-green/10 text-radar-green" : "bg-primary/10 text-primary"}`}>
                                        <Radio className="w-5 h-5" />
                                    </div>
                                    <div className="flex-1 min-w-0">
                                        <div className="flex items-center gap-2">
                                            <span className="font-mono text-sm font-bold text-foreground">{iface.name}</span>
                                            <span className={`font-mono text-[10px] uppercase tracking-widest px-2 py-0.5 rounded ${iface.mode === "monitor" ? "bg-radar-green/10 text-radar-green border border-radar-green/30" : "bg-primary/10 text-primary border border-primary/30"}`}>
                                                {iface.mode}
                                            </span>
                                            {iface.is_up && <span className="w-1.5 h-1.5 rounded-full bg-radar-green" />}
                                        </div>
                                        <div className="font-mono text-[10px] text-muted-foreground mt-1 flex gap-4">
                                            {iface.mac_address && <span>MAC: {iface.mac_address}</span>}
                                            {iface.driver && <span>Driver: {iface.driver}</span>}
                                            {iface.phy && <span>PHY: {iface.phy}</span>}
                                        </div>
                                    </div>
                                    <button
                                        onClick={() => handleToggleMode(iface.name)}
                                        disabled={toggleLoading === iface.name}
                                        className={`flex items-center gap-2 px-3 py-2 rounded font-mono text-[10px] uppercase tracking-widest border transition-all ${iface.mode === "monitor"
                                                ? "border-primary/40 text-primary hover:bg-primary hover:text-black"
                                                : "border-radar-green/40 text-radar-green hover:bg-radar-green hover:text-black"
                                            } disabled:opacity-30`}
                                    >
                                        {toggleLoading === iface.name ? (
                                            <RefreshCw className="w-3 h-3 animate-spin" />
                                        ) : iface.mode === "monitor" ? (
                                            <ToggleRight className="w-4 h-4" />
                                        ) : (
                                            <ToggleLeft className="w-4 h-4" />
                                        )}
                                        {iface.mode === "monitor" ? "Set Managed" : "Set Monitor"}
                                    </button>
                                </div>
                            ))}
                        </div>
                    )}
                </section>

                {/* Wired / Other Interfaces */}
                {wired.length > 0 && (
                    <section>
                        <h2 className="font-mono text-sm uppercase tracking-widest text-muted-foreground mb-3 flex items-center gap-2">
                            <ChevronRight className="w-4 h-4" />
                            Other Interfaces ({wired.length})
                        </h2>
                        <div className="space-y-2">
                            {wired.map((iface) => (
                                <div key={iface.name} className="glass-panel rounded-xl border border-border/20 p-3 flex items-center gap-3 opacity-60">
                                    <div className="w-8 h-8 rounded-lg bg-muted/30 flex items-center justify-center text-muted-foreground">
                                        <Wifi className="w-4 h-4" />
                                    </div>
                                    <div className="flex-1">
                                        <span className="font-mono text-xs text-foreground/70">{iface.name}</span>
                                        <div className="font-mono text-[10px] text-muted-foreground flex gap-3 mt-0.5">
                                            {iface.mac_address && <span>{iface.mac_address}</span>}
                                            <span>{iface.is_up ? "UP" : "DOWN"}</span>
                                        </div>
                                    </div>
                                </div>
                            ))}
                        </div>
                    </section>
                )}
            </div>
        </div>
    );
}
