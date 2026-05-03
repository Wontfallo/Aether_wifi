import {
    ShieldAlert, Zap, Download, Radio, Square, AlertTriangle, CheckCircle2,
    Loader2, FileDown, Octagon, Wifi, WifiOff, Shuffle, List, Music, Globe,
    Sparkles, Moon, Volume2, MessageSquareWarning, Send, Terminal, Play, Shield
} from "lucide-react";
import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { motion, AnimatePresence } from "framer-motion";
import type {
    DeauthResult,
    HandshakeResult,
    CaptureOperationStatus,
} from "../types/capture";
import { useBeaconCapture } from "../hooks/useBeaconCapture";
import {
    GlassCard, StatusBadge, ActionButton, PageHeader, TabBar, InputField, SelectField,
} from "../components/ui/shared";

function parseError(error: unknown): string {
    if (typeof error === "string") return error;
    if (error instanceof Error) return error.message;
    try {
        return JSON.stringify(error);
    } catch {
        return "Unknown error";
    }
}

type StrikeTab = "capture" | "spam" | "portal" | "advanced";
type AuditPhase = "idle" | "setup" | "deauth" | "capturing" | "complete" | "error" | "stopped";
type BeaconMode = "list" | "random" | "rickroll";
type RunningAttack =
    | null
    | "capture"
    | "beacon"
    | "probe"
    | "deauth"
    | "portal"
    | "karma"
    | "channel-switch"
    | "sleep"
    | "sae-flood"
    | "quiet-time"
    | "bad-message";

interface CapturedFile {
    path: string;
    bssid: string;
    timestamp: number;
}

export function Strike() {
    // --- Shared State ---
    const [tab, setTab] = useState<StrikeTab>("capture");
    const [targetBssid, setTargetBssid] = useState("");
    const [interfaceName, setInterfaceName] = useState("wlan0");
    const [running, setRunning] = useState<RunningAttack>(null);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    // --- Capture State ---
    const [deauthCount, setDeauthCount] = useState(5);
    const [deauthInterval, setDeauthInterval] = useState(100);
    const [phase, setPhase] = useState<AuditPhase>("idle");
    const [statusMessage, setStatusMessage] = useState("");
    const [progress, setProgress] = useState(0);
    const [capturedFiles, setCapturedFiles] = useState<CapturedFile[]>([]);
    const [packetsSent, setPacketsSent] = useState(0);
    const [packetsTotal, setPacketsTotal] = useState(0);

    const { beacons } = useBeaconCapture();

    // --- Spam/Flood State ---
    const [beaconMode, setBeaconMode] = useState<BeaconMode>("list");
    const [ssidText, setSsidText] = useState("FreeWiFi\nStarbucks_Guest\nxfinitywifi");
    const [randomCount, setRandomCount] = useState("50");

    // --- Portal State ---
    const [portalSsid, setPortalSsid] = useState("FreeWiFi");
    const [portalChannel, setPortalChannel] = useState("6");
    const [bettercapRunning, setBettercapRunning] = useState(false);
    const [bettercapCmd, setBettercapCmd] = useState("");
    const [cmdOutput, setCmdOutput] = useState<string[]>([]);
    const outputRef = useRef<HTMLDivElement>(null);

    // --- Advanced State ---
    const [advChannel, setAdvChannel] = useState("6");

    const clearError = () => setError(null);

    const isAttackActive = running !== null || (phase !== "idle" && phase !== "stopped" && phase !== "complete" && phase !== "error");

    // Listen for Capture events
    useEffect(() => {
        let unlistenEapol: UnlistenFn | null = null;
        let unlistenDeauth: UnlistenFn | null = null;

        listen<CaptureOperationStatus>("eapol-status", (event) => {
            const status = event.payload;
            setPhase(status.phase as AuditPhase);
            setStatusMessage(status.message);
            setProgress(status.progress);

            if (status.packets_sent !== undefined) setPacketsSent(status.packets_sent);
            if (status.packets_total !== undefined) setPacketsTotal(status.packets_total);

            if (status.phase === "complete") {
                setRunning(null);
                const pathMatch = status.message.match(/Saved to (.+)/);
                if (pathMatch) {
                    setCapturedFiles((prev) => [
                        ...prev,
                        { path: pathMatch[1], bssid: targetBssid || "Unknown", timestamp: Date.now() },
                    ]);
                }
            } else if (status.phase === "error") {
                setRunning(null);
                setError(status.message);
            } else if (status.phase === "stopped") {
                setRunning(null);
            }
        }).then((fn) => { unlistenEapol = fn; });

        listen<CaptureOperationStatus>("deauth-status", (event) => {
            const status = event.payload;
            setPhase(status.phase as AuditPhase);
            setStatusMessage(status.message);
            setProgress(status.progress);

            if (status.packets_sent !== undefined) setPacketsSent(status.packets_sent);
            if (status.packets_total !== undefined) setPacketsTotal(status.packets_total);

            if (status.phase === "complete" || status.phase === "stopped") {
                setRunning(null);
            } else if (status.phase === "error") {
                setRunning(null);
                setError(status.message);
            }
        }).then((fn) => { unlistenDeauth = fn; });

        return () => {
            if (unlistenEapol) unlistenEapol();
            if (unlistenDeauth) unlistenDeauth();
        };
    }, [targetBssid]);

    const runAttack = useCallback(async <T,>(name: RunningAttack, fn: () => Promise<T>) => {
        if (isAttackActive) return;
        setLoading(true);
        setError(null);
        try {
            await fn();
            setRunning(name);
        } catch (e) {
            setError(parseError(e));
        } finally {
            setLoading(false);
        }
    }, [isAttackActive]);

    const stopAllAttacks = useCallback(async (stopFn?: () => Promise<void>) => {
        setLoading(true);
        setError(null);
        try {
            if (stopFn) await stopFn();
            else await invoke("stop_all_attacks");
            setRunning(null);
            setBettercapRunning(false);
            if (phase !== "idle") {
               setPhase("idle");
               setStatusMessage("Attack stopped.");
            }
        } catch (e) {
            setError(parseError(e));
        } finally {
            setLoading(false);
        }
    }, [phase]);

    // --- Handlers ---
    const handleOneClickCapture = useCallback(async () => {
        if (!targetBssid) return setError("No target BSSID specified.");
        setPhase("setup");
        setStatusMessage("Initiating one-click capture...");
        setProgress(0.1);
        setPacketsSent(0);
        setPacketsTotal(deauthCount);
        runAttack("capture", () => invoke<HandshakeResult>("one_click_capture", {
            interfaceName, bssid: targetBssid, deauthCount, deauthIntervalMs: deauthInterval,
        }));
    }, [targetBssid, interfaceName, deauthCount, deauthInterval, runAttack]);

    const handleDeauthOnly = useCallback(async () => {
        if (!targetBssid) return setError("No target BSSID specified.");
        setPhase("deauth");
        setStatusMessage("Transmitting deauth frames...");
        setPacketsSent(0);
        setPacketsTotal(deauthCount);
        runAttack("deauth", () => invoke<DeauthResult>("start_deauth", {
            interfaceName, bssid: targetBssid, count: deauthCount, intervalMs: deauthInterval,
        }));
    }, [targetBssid, interfaceName, deauthCount, deauthInterval, runAttack]);

    const launchBeacon = () => {
        const params: Record<string, unknown> = { interfaceName, spamType: beaconMode };
        if (beaconMode === "list") params.ssids = ssidText.split("\n").map(s => s.trim()).filter(Boolean);
        return runAttack("beacon", () => invoke("start_beacon_spam", params));
    };

    const launchProbe = () => runAttack("probe", () => invoke("start_probe_flood", {
        interfaceName,
        bssid: targetBssid.trim() || undefined,
    }));
    const launchMdk4Deauth = () => runAttack("deauth", () => invoke("start_mdk4_deauth", { interfaceName }));

    const toggleBettercap = async () => {
        setLoading(true);
        setError(null);
        try {
            if (bettercapRunning) {
                await invoke("stop_bettercap_daemon");
                setBettercapRunning(false);
                setCmdOutput(prev => [...prev, ">> Bettercap stopped"]);
            } else {
                await invoke("start_bettercap_daemon", { interfaceName });
                setBettercapRunning(true);
                setCmdOutput(prev => [...prev, ">> Bettercap daemon started"]);
            }
        } catch (e) {
            setError(parseError(e));
        } finally {
            setLoading(false);
        }
    };

    const launchPortal = () => runAttack("portal", () => invoke("start_evil_portal", {
        ssid: portalSsid,
        channel: parseInt(portalChannel, 10),
    }));
    const launchKarma = () => runAttack("karma", () => invoke("start_karma_attack", {
        channel: parseInt(portalChannel, 10),
    }));

    const sendBettercapCmd = async () => {
        if (!bettercapCmd.trim()) return;
        setCmdOutput(prev => [...prev, `> ${bettercapCmd}`]);
        try {
            const result = await invoke<string>("bettercap_command", { cmd: bettercapCmd });
            setCmdOutput(prev => [...prev, result]);
        } catch (e) {
            setCmdOutput(prev => [...prev, `[ERROR] ${parseError(e)}`]);
        }
        setBettercapCmd("");
        setTimeout(() => outputRef.current?.scrollTo(0, outputRef.current.scrollHeight), 50);
    };

    const advancedAttacks = [
        { id: "channel-switch" as const, name: "Channel Switch", desc: "Force clients to switch channels", icon: <Shuffle size={20} />, needsBssid: true, needsChannel: true, invoke: async () => { await invoke("start_channel_switch", { interfaceName, targetBssid, targetChannel: parseInt(advChannel, 10) }); } },
        { id: "sleep" as const, name: "Sleep Attack", desc: "Send power-save frames", icon: <Moon size={20} />, needsBssid: true, needsChannel: false, invoke: async () => { await invoke("start_sleep_attack", { interfaceName, targetBssid }); } },
        { id: "sae-flood" as const, name: "SAE Flood", desc: "Flood WPA3 SAE auth", icon: <Sparkles size={20} />, needsBssid: true, needsChannel: false, invoke: async () => { await invoke("start_sae_flood", { interfaceName, targetBssid }); } },
        { id: "quiet-time" as const, name: "Quiet Time", desc: "Inject quiet period elements", icon: <Volume2 size={20} />, needsBssid: false, needsChannel: true, invoke: async () => { await invoke("start_quiet_time", { interfaceName, channel: parseInt(advChannel, 10), durationMs: 100 }); } },
        { id: "bad-message" as const, name: "Bad Message", desc: "Malformed management frames", icon: <MessageSquareWarning size={20} />, needsBssid: true, needsChannel: false, invoke: async () => { await invoke("start_bad_message", { interfaceName, targetBssid }); } },
    ];

    const channelOptions = Array.from({ length: 14 }, (_, i) => ({ value: String(i + 1), label: `Channel ${i + 1}` }));

    const tabs = [
        { id: "capture", label: "Capture", icon: <Zap size={14} /> },
        { id: "spam", label: "Spam/Flood", icon: <Radio size={14} /> },
        { id: "portal", label: "Portals", icon: <Globe size={14} /> },
        { id: "advanced", label: "Advanced", icon: <Octagon size={14} /> },
    ];

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

    return (
        <div className="h-full flex flex-col animate-in fade-in slide-in-from-bottom-4 duration-500">
            <PageHeader
                icon={<ShieldAlert size={28} className="text-destructive" />}
                title="STRIKE"
                subtitle="OFFENSIVE OPS"
                description="Authorized Use Only"
                accent="text-destructive"
            >
                {isAttackActive ? (
                    <StatusBadge status="active" label={running ? `${running.toUpperCase()} ACTIVE` : "ATTACK ACTIVE"} pulse />
                ) : (
                    <StatusBadge status="inactive" label="READY" />
                )}
            </PageHeader>

            {/* Global Error Banner */}
            <AnimatePresence>
                {error && (
                    <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }}
                        className="mb-4 p-3 border border-destructive/50 bg-destructive/10 text-destructive rounded-lg font-mono text-xs flex items-center gap-2 cursor-pointer"
                        onClick={clearError}>
                        <AlertTriangle className="w-4 h-4 shrink-0" /> {error}
                        <button className="ml-auto opacity-60 hover:opacity-100">×</button>
                    </motion.div>
                )}
            </AnimatePresence>

            {/* Global Active Attack Banner */}
            {isAttackActive && (
                <div className="mb-4 p-3 glass-panel rounded-lg border border-radar-alert/40 bg-radar-alert/5 flex flex-col gap-2">
                    <div className="flex items-center justify-between">
                        <div className="flex items-center gap-3">
                            {running === "capture" || phase !== "idle" ? <PhaseIcon p={phase} /> : <Radio className="w-4 h-4 text-radar-alert animate-pulse" />}
                            <div>
                               <span className={`font-mono text-xs uppercase tracking-widest font-bold block ${phaseColor(phase)}`}>
                                   {running ? `Running: ${running}` : `Phase: ${phase}`}
                               </span>
                               {statusMessage && <span className="font-mono text-[10px] text-muted-foreground">{statusMessage}</span>}
                            </div>
                        </div>
                        <button onClick={() => stopAllAttacks()} className="bg-destructive hover:bg-destructive/80 text-white px-4 py-1.5 rounded font-mono text-xs uppercase tracking-widest flex items-center gap-2 shadow-[0_0_15px_rgba(239,68,68,0.4)] transition-all">
                            <Square className="w-3 h-3" /> STOP ALL
                        </button>
                    </div>
                    {/* Progress Bar */}
                    {phase !== "idle" && (
                        <div>
                            <div className="w-full h-1 bg-black/40 rounded-full overflow-hidden mt-1">
                                <div
                                    className={`h-full rounded-full transition-all duration-500 ${phase === "error" ? "bg-destructive" : phase === "complete" ? "bg-radar-green" : phase === "stopped" ? "bg-orange-500" : "bg-radar-yellow"}`}
                                    style={{ width: `${progress * 100}%` }}
                                />
                            </div>
                            {packetsTotal > 0 && (
                                <div className="mt-1 font-mono text-[10px] text-muted-foreground">
                                    Packets: {packetsSent} / {packetsTotal}
                                </div>
                            )}
                        </div>
                    )}
                </div>
            )}

            {/* Main Content Area */}
            <div className="grid grid-cols-1 lg:grid-cols-4 gap-6 flex-1 min-h-0">
                {/* LEFT: Shared Target Config */}
                <div className="lg:col-span-1 glass-panel rounded-xl border border-radar-yellow/30 p-5 flex flex-col gap-4 overflow-auto">
                    <h2 className="font-mono text-sm tracking-widest uppercase text-radar-yellow border-b border-radar-yellow/20 pb-2">Target Config</h2>
                    <InputField label="Interface" value={interfaceName} onChange={setInterfaceName} placeholder="wlan0" />
                    <div>
                        <InputField label="Target BSSID (blank = broadcast)" value={targetBssid} onChange={(v) => setTargetBssid(v.toUpperCase())} placeholder="AA:BB:CC:DD:EE:FF" className="uppercase" />
                    </div>
                    
                    {tab === "capture" && (
                        <>
                            <InputField label="Deauth Count" value={String(deauthCount)} onChange={(v) => setDeauthCount(parseInt(v) || 5)} type="number" />
                            <InputField label="Interval (ms)" value={String(deauthInterval)} onChange={(v) => setDeauthInterval(parseInt(v) || 100)} type="number" />
                        </>
                    )}
                    {tab === "portal" && (
                        <>
                            <InputField label="Portal SSID" value={portalSsid} onChange={setPortalSsid} placeholder="FreeWiFi" />
                            <SelectField label="Portal Channel" value={portalChannel} onChange={setPortalChannel} options={channelOptions} />
                        </>
                    )}
                    {tab === "advanced" && (
                        <SelectField label="Target Channel" value={advChannel} onChange={setAdvChannel} options={channelOptions} />
                    )}

                    {beacons.size > 0 && (
                        <div className="mt-2">
                            <label className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground mb-1 block">Quick Select</label>
                            <div className="max-h-32 overflow-auto bg-black/20 rounded border border-border/30">
                                {Array.from(beacons.values()).slice(0, 20).map((b) => (
                                    <button key={b.bssid} onClick={() => setTargetBssid(b.bssid)}
                                        className={`w-full text-left px-2 py-1 font-mono text-[10px] hover:bg-radar-yellow/10 transition-colors border-b border-border/10 ${targetBssid === b.bssid ? "text-radar-yellow bg-radar-yellow/5" : "text-muted-foreground"}`}>
                                        <span className="opacity-70">{b.bssid}</span> <span className="text-foreground/80">{b.ssid || "<hidden>"}</span>
                                    </button>
                                ))}
                            </div>
                        </div>
                    )}
                </div>

                {/* RIGHT: Tabs Content */}
                <div className="lg:col-span-3 flex flex-col min-h-0">
                    <TabBar tabs={tabs} active={tab} onChange={(id) => setTab(id as StrikeTab)} />
                    <div className="flex-1 min-h-0 mt-4 overflow-auto">
                        
                        {/* TAB: CAPTURE */}
                        {tab === "capture" && (
                            <motion.div initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} className="h-full flex flex-col gap-4">
                                <div className="grid grid-cols-2 gap-4">
                                    <button onClick={handleOneClickCapture} disabled={isAttackActive || !targetBssid}
                                        className="bg-radar-yellow/10 border border-radar-yellow/50 text-radar-yellow hover:bg-radar-yellow hover:text-black transition-all rounded py-4 font-mono text-sm font-bold uppercase tracking-widest flex items-center justify-center gap-2 shadow-[0_0_15px_rgba(255,234,0,0.1)] disabled:opacity-30 disabled:cursor-not-allowed">
                                        <Zap className="w-5 h-5" /> 1-Click Capture
                                    </button>
                                    <button onClick={handleDeauthOnly} disabled={isAttackActive || !targetBssid}
                                        className="bg-destructive/10 border border-destructive/40 text-destructive/80 hover:bg-destructive hover:text-white transition-all rounded py-4 font-mono text-sm uppercase tracking-widest disabled:opacity-30 disabled:cursor-not-allowed">
                                        Deauth Only
                                    </button>
                                </div>

                                <div className="glass-panel rounded-xl border border-border/40 p-5 flex flex-col flex-1 min-h-0">
                                    <h2 className="font-mono text-sm tracking-widest uppercase text-muted-foreground mb-4 flex justify-between items-center border-b border-border/30 pb-3">
                                        <span className="flex items-center gap-2"><FileDown className="w-4 h-4" /> Loot / PCAPs</span>
                                        <span className="text-[10px] bg-muted px-2 py-0.5 rounded text-foreground">{capturedFiles.length} File{capturedFiles.length !== 1 ? "s" : ""}</span>
                                    </h2>
                                    {capturedFiles.length === 0 ? (
                                        <div className="flex-1 border border-dashed border-border/40 rounded-lg flex flex-col items-center justify-center text-center gap-3 bg-black/20 p-8">
                                            <Download className="w-10 h-10 text-muted-foreground/30" />
                                            <p className="font-mono text-xs uppercase tracking-widest text-muted-foreground/50">No handshakes captured yet.</p>
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
                                                        <tr key={i} className="border-b border-border/10 hover:bg-white/5">
                                                            <td className="py-2 px-4 text-radar-yellow">{file.bssid}</td>
                                                            <td className="py-2 px-4 text-foreground/70 truncate">{file.path}</td>
                                                            <td className="py-2 px-4 text-muted-foreground text-right">{new Date(file.timestamp).toLocaleTimeString()}</td>
                                                        </tr>
                                                    ))}
                                                </tbody>
                                            </table>
                                        </div>
                                    )}
                                </div>
                            </motion.div>
                        )}

                        {/* TAB: SPAM/FLOOD */}
                        {tab === "spam" && (
                            <motion.div initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} className="space-y-4">
                                <GlassCard accent="destructive" className="p-6">
                                    <h3 className="font-mono text-sm uppercase tracking-widest text-foreground mb-4 flex items-center gap-2"><Radio size={16} className="text-destructive" /> Beacon Spam Mode</h3>
                                    <div className="flex gap-3 mb-6">
                                        {(["list", "random", "rickroll"] as BeaconMode[]).map((m) => (
                                            <button key={m} onClick={() => setBeaconMode(m)} className={`flex items-center gap-2 px-4 py-2 rounded font-mono text-xs uppercase tracking-widest border transition-all ${beaconMode === m ? "bg-destructive/15 border-destructive/40 text-destructive" : "bg-black/20 border-border/40 text-muted-foreground"}`}>
                                                {m === "list" && <List size={14} />} {m === "random" && <Shuffle size={14} />} {m === "rickroll" && <Music size={14} />} {m}
                                            </button>
                                        ))}
                                    </div>
                                    {beaconMode === "list" && <textarea value={ssidText} onChange={(e) => setSsidText(e.target.value)} rows={5} className="w-full bg-black/40 border border-border/60 rounded px-4 py-2 font-mono text-sm text-foreground" />}
                                    {beaconMode === "random" && <InputField label="Count" value={randomCount} onChange={setRandomCount} type="number" />}
                                    
                                    <div className="mt-6 flex items-center gap-4">
                                        <ActionButton variant="destructive" size="lg" onClick={launchBeacon} loading={loading} disabled={isAttackActive} className="flex-1"><Play size={16} /> LAUNCH BEACON SPAM</ActionButton>
                                    </div>
                                </GlassCard>

                                <GlassCard accent="destructive" className="p-6">
                                    <h3 className="font-mono text-sm uppercase tracking-widest text-foreground mb-4 flex items-center gap-2"><WifiOff size={16} className="text-destructive" /> Floods</h3>
                                    <div className="flex gap-4">
                                        <ActionButton variant="destructive" size="lg" onClick={launchProbe} loading={loading} disabled={isAttackActive} className="flex-1"><Wifi size={16} /> PROBE FLOOD</ActionButton>
                                        <ActionButton variant="destructive" size="lg" onClick={launchMdk4Deauth} loading={loading} disabled={isAttackActive} className="flex-1"><WifiOff size={16} /> BROADCAST DEAUTH</ActionButton>
                                    </div>
                                </GlassCard>
                            </motion.div>
                        )}

                        {/* TAB: PORTALS */}
                        {tab === "portal" && (
                            <motion.div initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} className="grid grid-cols-2 gap-4 h-full">
                                <GlassCard accent="destructive" className="p-6 space-y-4 h-fit">
                                    <h3 className="font-mono text-sm uppercase tracking-widest text-foreground flex items-center gap-2"><Globe size={16} className="text-destructive" /> Portal Control</h3>
                                    <ActionButton variant={bettercapRunning ? "ghost" : "primary"} size="md" onClick={toggleBettercap} loading={loading} className="w-full justify-center">
                                        {bettercapRunning ? <><Square size={14} /> Stop Bettercap</> : <><Play size={14} /> Start Bettercap Daemon</>}
                                    </ActionButton>
                                    <div className="flex gap-2 pt-2">
                                        <ActionButton variant="destructive" size="md" onClick={launchPortal} loading={loading} disabled={isAttackActive || !bettercapRunning} className="flex-1 justify-center"><Shield size={14} /> Evil Portal</ActionButton>
                                        <ActionButton variant="destructive" size="md" onClick={launchKarma} loading={loading} disabled={isAttackActive || !bettercapRunning} className="flex-1 justify-center"><Sparkles size={14} /> Karma</ActionButton>
                                    </div>
                                </GlassCard>

                                <GlassCard accent="none" className="p-6 flex flex-col h-full min-h-[300px]">
                                    <h3 className="font-mono text-sm uppercase tracking-widest text-foreground flex items-center gap-2 mb-4"><Terminal size={16} className="text-primary" /> Bettercap Console</h3>
                                    <div ref={outputRef} className="flex-1 bg-black/60 rounded border border-border/40 p-3 overflow-y-auto font-mono text-[11px] text-radar-green/80 space-y-0.5">
                                        {cmdOutput.length === 0 ? <span className="text-muted-foreground italic">No output yet...</span> : cmdOutput.map((line, i) => <div key={i} className={line.startsWith("[ERROR]") ? "text-destructive" : line.startsWith(">>") ? "text-primary" : ""}>{line}</div>)}
                                    </div>
                                    <div className="flex gap-2 mt-3">
                                        <input value={bettercapCmd} onChange={(e) => setBettercapCmd(e.target.value)} onKeyDown={(e) => e.key === "Enter" && sendBettercapCmd()} placeholder={bettercapRunning ? "wifi.show" : "Start bettercap first..."} disabled={!bettercapRunning} className="flex-1 bg-black/40 border border-border/60 rounded px-3 py-2 font-mono text-xs text-foreground focus:outline-none" />
                                        <ActionButton variant="primary" size="sm" onClick={sendBettercapCmd} disabled={!bettercapRunning || !bettercapCmd.trim()}><Send size={14} /></ActionButton>
                                    </div>
                                </GlassCard>
                            </motion.div>
                        )}

                        {/* TAB: ADVANCED */}
                        {tab === "advanced" && (
                            <motion.div initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} className="grid grid-cols-2 gap-4">
                                {advancedAttacks.map((atk) => {
                                    const disabled = isAttackActive || (atk.needsBssid && !targetBssid.trim());
                                    return (
                                        <GlassCard key={atk.id} accent="destructive" className="p-5">
                                            <div className="flex items-start justify-between mb-3">
                                                <div className="flex items-center gap-3">
                                                    <div className="w-9 h-9 rounded-lg bg-destructive/10 border border-destructive/20 flex items-center justify-center text-destructive">{atk.icon}</div>
                                                    <div>
                                                        <h4 className="font-mono text-sm font-bold text-foreground uppercase tracking-wider">{atk.name}</h4>
                                                        <p className="text-[11px] text-muted-foreground mt-0.5">{atk.desc}</p>
                                                    </div>
                                                </div>
                                            </div>
                                            <div className="flex items-center gap-2 text-[10px] font-mono text-muted-foreground mb-3">
                                                {atk.needsBssid && <span className={`px-2 py-0.5 rounded border ${targetBssid.trim() ? "border-radar-green/30 text-radar-green" : "border-destructive/30 text-destructive"}`}>BSSID {targetBssid.trim() ? "✓" : "Required"}</span>}
                                                {atk.needsChannel && <span className="px-2 py-0.5 rounded border border-primary/30 text-primary">CH {advChannel}</span>}
                                            </div>
                                            <ActionButton variant="destructive" size="sm" onClick={() => runAttack(atk.id, atk.invoke)} loading={loading} disabled={disabled} className="w-full justify-center"><Play size={14} /> LAUNCH</ActionButton>
                                        </GlassCard>
                                    );
                                })}
                            </motion.div>
                        )}
                    </div>
                </div>
            </div>
        </div>
    );
}
