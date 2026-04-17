import { ShieldAlert, Zap, Download, Radio, Square, AlertTriangle, CheckCircle2, Loader2, FileDown, Octagon } from "lucide-react";
import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
    DeauthResult,
    HandshakeResult,
    CaptureOperationStatus,
} from "../types/capture";
import { useBeaconCapture } from "../hooks/useBeaconCapture";

type AuditPhase = "idle" | "setup" | "deauth" | "capturing" | "complete" | "error" | "stopped";

interface CapturedFile {
    path: string;
    bssid: string;
    timestamp: number;
}

export function Audit() {
    const [targetBssid, setTargetBssid] = useState("");
    const [interfaceName, setInterfaceName] = useState("wlan0");
    const [deauthCount, setDeauthCount] = useState(5);
    const [deauthInterval, setDeauthInterval] = useState(100); // milliseconds between deauth packets
    const [phase, setPhase] = useState<AuditPhase>("idle");
    const [statusMessage, setStatusMessage] = useState("");
    const [progress, setProgress] = useState(0);
    const [capturedFiles, setCapturedFiles] = useState<CapturedFile[]>([]);
    const [error, setError] = useState<string | null>(null);
    const [isEapolCapturing, setIsEapolCapturing] = useState(false);
    const [isDeauthing, setIsDeauthing] = useState(false);
    const [packetsSent, setPacketsSent] = useState(0);
    const [packetsTotal, setPacketsTotal] = useState(0);

    // Use beacon capture to populate target list
    const { beacons } = useBeaconCapture();

    // Listen for EAPOL status events from the backend
    useEffect(() => {
        let unlistenEapol: UnlistenFn | null = null;
        let unlistenDeauth: UnlistenFn | null = null;

        listen<CaptureOperationStatus>("eapol-status", (event) => {
            const status = event.payload;
            setPhase(status.phase as AuditPhase);
            setStatusMessage(status.message);
            setProgress(status.progress);

            if (status.packets_sent !== undefined) {
                setPacketsSent(status.packets_sent);
            }
            if (status.packets_total !== undefined) {
                setPacketsTotal(status.packets_total);
            }

            if (status.phase === "complete") {
                setIsEapolCapturing(false);
                setIsDeauthing(false);
                // Extract pcap path from message if present
                const pathMatch = status.message.match(/Saved to (.+)/);
                if (pathMatch) {
                    setCapturedFiles((prev) => [
                        ...prev,
                        {
                            path: pathMatch[1],
                            bssid: targetBssid || "Unknown",
                            timestamp: Date.now(),
                        },
                    ]);
                }
            } else if (status.phase === "error") {
                setIsEapolCapturing(false);
                setIsDeauthing(false);
                setError(status.message);
            } else if (status.phase === "stopped") {
                setIsEapolCapturing(false);
                setIsDeauthing(false);
            }
        }).then((fn) => {
            unlistenEapol = fn;
        });

        // Listen for deauth status events
        listen<CaptureOperationStatus>("deauth-status", (event) => {
            const status = event.payload;
            setPhase(status.phase as AuditPhase);
            setStatusMessage(status.message);
            setProgress(status.progress);

            if (status.packets_sent !== undefined) {
                setPacketsSent(status.packets_sent);
            }
            if (status.packets_total !== undefined) {
                setPacketsTotal(status.packets_total);
            }

            if (status.phase === "complete" || status.phase === "stopped") {
                setIsDeauthing(false);
            } else if (status.phase === "error") {
                setIsDeauthing(false);
                setError(status.message);
            }
        }).then((fn) => {
            unlistenDeauth = fn;
        });

        return () => {
            if (unlistenEapol) unlistenEapol();
            if (unlistenDeauth) unlistenDeauth();
        };
    }, [targetBssid]);

    // One-click capture handler
    const handleOneClickCapture = useCallback(async () => {
        if (!targetBssid) {
            setError("No target BSSID specified.");
            return;
        }

        setError(null);
        setPhase("setup");
        setStatusMessage("Initiating one-click capture...");
        setProgress(0.1);
        setIsEapolCapturing(true);
        setIsDeauthing(true);
        setPacketsSent(0);
        setPacketsTotal(deauthCount);

        try {
            await invoke<HandshakeResult>("one_click_capture", {
                interfaceName,
                bssid: targetBssid,
                deauthCount,
                deauthIntervalMs: deauthInterval,
            });
        } catch (err: unknown) {
            const msg = typeof err === "string" ? err : JSON.stringify(err);
            setError(msg);
            setPhase("error");
            setIsEapolCapturing(false);
            setIsDeauthing(false);
        }
    }, [targetBssid, interfaceName, deauthCount, deauthInterval]);

    // Manual deauth-only handler
    const handleDeauthOnly = useCallback(async () => {
        if (!targetBssid) {
            setError("No target BSSID specified.");
            return;
        }

        setError(null);
        setPhase("deauth");
        setStatusMessage("Transmitting deauth frames...");
        setIsDeauthing(true);
        setPacketsSent(0);
        setPacketsTotal(deauthCount);

        try {
            await invoke<DeauthResult>("start_deauth", {
                interfaceName,
                bssid: targetBssid,
                count: deauthCount,
                intervalMs: deauthInterval,
            });
        } catch (err: unknown) {
            const msg = typeof err === "string" ? err : JSON.stringify(err);
            setError(msg);
            setPhase("error");
            setIsDeauthing(false);
        }
    }, [targetBssid, interfaceName, deauthCount, deauthInterval]);

    // Stop all attacks
    const handleStopAll = useCallback(async () => {
        try {
            await invoke<HandshakeResult>("stop_all_attacks");
            setIsEapolCapturing(false);
            setIsDeauthing(false);
            setPhase("idle");
            setStatusMessage("Attack stopped.");
        } catch (err: unknown) {
            const msg = typeof err === "string" ? err : JSON.stringify(err);
            setError(msg);
        }
    }, []);

    // Select a target from the beacon list
    const selectTarget = (bssid: string) => {
        setTargetBssid(bssid);
    };

    const phaseColor = (p: AuditPhase) => {
        switch (p) {
            case "idle": return "text-muted-foreground";
            case "setup": return "text-primary";
            case "deauth": return "text-radar-yellow";
            case "capturing": return "text-radar-alert";
            case "complete": return "text-radar-green";
            case "error": return "text-destructive";
            case "stopped": return "text-orange-500";
        }
    };

    const PhaseIcon = ({ p }: { p: AuditPhase }) => {
        switch (p) {
            case "idle": return <Radio className="w-4 h-4" />;
            case "setup":
            case "deauth":
            case "capturing": return <Loader2 className="w-4 h-4 animate-spin" />;
            case "complete": return <CheckCircle2 className="w-4 h-4" />;
            case "error": return <AlertTriangle className="w-4 h-4" />;
            case "stopped": return <Octagon className="w-4 h-4" />;
        }
    };

    const isAttackActive = isEapolCapturing || isDeauthing;

    return (
        <div className="h-full flex flex-col animate-in fade-in slide-in-from-bottom-4 duration-500">
            <header className="mb-6">
                <h1 className="text-3xl font-mono font-bold tracking-tight text-warning flex items-center gap-3">
                    <ShieldAlert className="w-8 h-8 text-radar-yellow" />
                    <span className="text-glow text-radar-yellow" style={{ textShadow: "0 0 10px rgba(255,234,0,0.5)" }}>AUDIT</span>{" "}
                    <span className="opacity-50 text-muted-foreground font-sans text-2xl font-normal">// OFFENSIVE SUITE</span>
                </h1>
                <p className="text-muted-foreground mt-2 font-mono text-sm uppercase tracking-wider">Authorized Use Only</p>
            </header>

            {/* Error Banner */}
            {error && (
                <div className="mb-4 p-3 border border-destructive/50 bg-destructive/10 text-destructive rounded-lg font-mono text-xs flex items-center gap-2">
                    <AlertTriangle className="w-4 h-4 shrink-0" />
                    {error}
                    <button onClick={() => setError(null)} className="ml-auto opacity-60 hover:opacity-100">×</button>
                </div>
            )}

            {/* Status Bar */}
            {phase !== "idle" && (
                <div className="mb-4 p-3 glass-panel rounded-lg border border-border/40">
                    <div className="flex items-center gap-3 mb-2">
                        <PhaseIcon p={phase} />
                        <span className={`font-mono text-xs uppercase tracking-widest ${phaseColor(phase)}`}>{phase}</span>
                        <span className="font-mono text-xs text-muted-foreground ml-2">{statusMessage}</span>
                    </div>
                    <div className="w-full h-1 bg-black/40 rounded-full overflow-hidden">
                        <div
                            className={`h-full rounded-full transition-all duration-500 ${phase === "error" ? "bg-destructive" : phase === "complete" ? "bg-radar-green" : phase === "stopped" ? "bg-orange-500" : "bg-radar-yellow"}`}
                            style={{ width: `${progress * 100}%` }}
                        />
                    </div>
                    {packetsTotal > 0 && (
                        <div className="mt-2 font-mono text-[10px] text-muted-foreground">
                            Packets: {packetsSent} / {packetsTotal}
                        </div>
                    )}
                </div>
            )}

            <div className="grid grid-cols-1 lg:grid-cols-3 gap-6 flex-1 min-h-0">
                {/* LEFT: Target Config */}
                <div className="lg:col-span-1 glass-panel rounded-xl border border-radar-yellow/30 p-5 flex flex-col gap-4 overflow-auto">
                    <h2 className="font-mono text-sm tracking-widest uppercase text-radar-yellow border-b border-radar-yellow/20 pb-2">Target Config</h2>

                    <div>
                        <label className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground mb-1 block">Interface</label>
                        <input
                            type="text"
                            value={interfaceName}
                            onChange={(e) => setInterfaceName(e.target.value)}
                            className="w-full bg-black/40 border border-border/60 rounded px-3 py-2 font-mono text-sm text-foreground focus:border-radar-yellow focus:outline-none transition-colors"
                        />
                    </div>

                    <div>
                        <label className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground mb-1 block">Target BSSID</label>
                        <input
                            type="text"
                            placeholder="AA:BB:CC:DD:EE:FF"
                            value={targetBssid}
                            onChange={(e) => setTargetBssid(e.target.value.toUpperCase())}
                            className="w-full bg-black/40 border border-border/60 rounded px-3 py-2 font-mono text-sm text-radar-yellow focus:border-radar-yellow focus:outline-none transition-colors uppercase"
                        />
                    </div>

                    <div>
                        <label className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground mb-1 block">Deauth Count</label>
                        <input
                            type="number"
                            min={1}
                            max={100}
                            value={deauthCount}
                            onChange={(e) => setDeauthCount(parseInt(e.target.value) || 5)}
                            className="w-full bg-black/40 border border-border/60 rounded px-3 py-2 font-mono text-sm text-foreground focus:border-radar-yellow focus:outline-none transition-colors"
                        />
                    </div>

                    <div>
                        <label className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground mb-1 block">Interval (ms)</label>
                        <input
                            type="number"
                            min={10}
                            max={10000}
                            step={10}
                            value={deauthInterval}
                            onChange={(e) => setDeauthInterval(parseInt(e.target.value) || 100)}
                            className="w-full bg-black/40 border border-border/60 rounded px-3 py-2 font-mono text-sm text-foreground focus:border-radar-yellow focus:outline-none transition-colors"
                        />
                        <p className="text-[9px] text-muted-foreground mt-1">Time between each deauth packet</p>
                    </div>

                    {/* Discovered Targets (from beacon capture) */}
                    {beacons.size > 0 && (
                        <div>
                            <label className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground mb-1 block">Quick Select Target</label>
                            <div className="max-h-32 overflow-auto bg-black/20 rounded border border-border/30">
                                {Array.from(beacons.values()).slice(0, 20).map((b) => (
                                    <button
                                        key={b.bssid}
                                        onClick={() => selectTarget(b.bssid)}
                                        className={`w-full text-left px-2 py-1 font-mono text-[10px] hover:bg-radar-yellow/10 transition-colors border-b border-border/10 ${targetBssid === b.bssid ? "text-radar-yellow bg-radar-yellow/5" : "text-muted-foreground"}`}
                                    >
                                        <span className="opacity-70">{b.bssid}</span> <span className="text-foreground/80">{b.ssid || "<hidden>"}</span>
                                    </button>
                                ))}
                            </div>
                        </div>
                    )}

                    {/* Action Buttons */}
                    <div className="mt-auto space-y-3 pt-4">
                        {/* STOP ATTACK BUTTON - Prominent when attack is active */}
                        {isAttackActive && (
                            <button
                                onClick={handleStopAll}
                                className="w-full bg-destructive border-2 border-destructive text-white hover:bg-destructive/80 transition-all rounded py-3 font-mono text-xs font-bold uppercase tracking-widest flex items-center justify-center gap-2 shadow-[0_0_20px_rgba(239,68,68,0.4)] animate-pulse"
                            >
                                <Octagon className="w-5 h-5" />
                                STOP ATTACK
                            </button>
                        )}

                        <button
                            onClick={handleOneClickCapture}
                            disabled={isAttackActive || !targetBssid}
                            className="w-full bg-radar-yellow/10 border border-radar-yellow/50 text-radar-yellow hover:bg-radar-yellow hover:text-black transition-all rounded py-3 font-mono text-xs font-bold uppercase tracking-widest flex items-center justify-center gap-2 shadow-[0_0_15px_rgba(255,234,0,0.1)] hover:shadow-[0_0_20px_rgba(255,234,0,0.4)] disabled:opacity-30 disabled:cursor-not-allowed disabled:hover:bg-radar-yellow/10 disabled:hover:text-radar-yellow"
                        >
                            <Zap className="w-4 h-4" />
                            {isEapolCapturing ? "Capturing..." : "1-Click Capture"}
                        </button>

                        <div className="grid grid-cols-2 gap-2">
                            <button
                                onClick={handleDeauthOnly}
                                disabled={isAttackActive || !targetBssid}
                                className="bg-destructive/10 border border-destructive/40 text-destructive/80 hover:bg-destructive hover:text-white transition-all rounded py-2 font-mono text-[10px] tracking-widest uppercase disabled:opacity-30 disabled:cursor-not-allowed"
                            >
                                Deauth Only
                            </button>
                            <button
                                onClick={handleStopAll}
                                disabled={!isAttackActive}
                                className="bg-orange-500/10 border border-orange-500/40 text-orange-500 hover:bg-orange-500 hover:text-white transition-all rounded py-2 font-mono text-[10px] tracking-widest uppercase disabled:opacity-30 disabled:cursor-not-allowed"
                            >
                                <Square className="w-3 h-3 inline mr-1" />
                                Stop
                            </button>
                        </div>
                    </div>
                </div>

                {/* RIGHT: Captured PCAPs / Loot */}
                <div className="lg:col-span-2 glass-panel rounded-xl border border-border/40 p-5 flex flex-col">
                    <h2 className="font-mono text-sm tracking-widest uppercase text-muted-foreground mb-4 flex justify-between items-center border-b border-border/30 pb-3">
                        <span className="flex items-center gap-2">
                            <FileDown className="w-4 h-4" />
                            Loot / Captured PCAPs
                        </span>
                        <span className="text-[10px] bg-muted px-2 py-0.5 rounded text-foreground">{capturedFiles.length} File{capturedFiles.length !== 1 ? "s" : ""}</span>
                    </h2>

                    {capturedFiles.length === 0 ? (
                        <div className="flex-1 border border-dashed border-border/40 rounded-lg flex flex-col items-center justify-center text-center gap-3 bg-black/20">
                            <div className="relative">
                                <Download className="w-10 h-10 text-muted-foreground/30" />
                                {isEapolCapturing && (
                                    <div className="absolute inset-0 flex items-center justify-center">
                                        <Loader2 className="w-6 h-6 text-radar-yellow animate-spin" />
                                    </div>
                                )}
                            </div>
                            <p className="font-mono text-xs uppercase tracking-widest text-muted-foreground/50">
                                {isEapolCapturing ? (
                                    <>Listening for EAPOL packets...<br />Waiting for WPA handshake.</>
                                ) : (
                                    <>No handshakes captured yet.<br />Target a BSSID and initiate capture.</>
                                )}
                            </p>
                        </div>
                    ) : (
                        <div className="flex-1 overflow-auto bg-black/20 rounded-lg border border-border/20">
                            <table className="w-full text-left font-mono text-xs border-collapse">
                                <thead className="sticky top-0 bg-background/95 backdrop-blur z-10 border-b border-border/40">
                                    <tr>
                                        <th className="py-2 px-4 font-normal text-muted-foreground uppercase tracking-widest">BSSID</th>
                                        <th className="py-2 px-4 font-normal text-muted-foreground uppercase tracking-widest">File Path</th>
                                        <th className="py-2 px-4 font-normal text-muted-foreground uppercase tracking-widest text-right">Time</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {capturedFiles.map((file, i) => (
                                        <tr key={i} className="border-b border-border/10 hover:bg-white/5 transition-colors">
                                            <td className="py-2 px-4 text-radar-yellow">{file.bssid}</td>
                                            <td className="py-2 px-4 text-foreground/70 truncate max-w-xs">{file.path}</td>
                                            <td className="py-2 px-4 text-muted-foreground text-right">
                                                {new Date(file.timestamp).toLocaleTimeString()}
                                            </td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
}
