import {
  Zap, Radio, Skull, Shield, Send, Terminal, Play, Square,
  Wifi, WifiOff, AlertTriangle, Shuffle, List, Music,
  Globe, Sparkles, Moon, Volume2, MessageSquareWarning
} from "lucide-react";
import { useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";
import {
  GlassCard, StatusBadge, ActionButton, PageHeader, TabBar, InputField, SelectField,
} from "../components/ui/shared";

type AttackTab = "beacon" | "deauth" | "portal" | "advanced";
type BeaconMode = "list" | "random" | "rickroll";
type RunningAttack =
  | null
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

export function Attack() {
  const [tab, setTab] = useState<AttackTab>("beacon");
  const [running, setRunning] = useState<RunningAttack>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Beacon state
  const [beaconMode, setBeaconMode] = useState<BeaconMode>("list");
  const [ssidText, setSsidText] = useState("FreeWiFi\nStarbucks_Guest\nxfinitywifi");
  const [randomCount, setRandomCount] = useState("50");
  const [beaconIface, setBeaconIface] = useState("wlan0");

  // Deauth state
  const [deauthBssid, setDeauthBssid] = useState("");
  const [deauthIface, setDeauthIface] = useState("wlan0");

  // Portal state
  const [portalSsid, setPortalSsid] = useState("FreeWiFi");
  const [portalChannel, setPortalChannel] = useState("6");
  const [portalIface, setPortalIface] = useState("wlan0");
  const [bettercapRunning, setBettercapRunning] = useState(false);
  const [bettercapCmd, setBettercapCmd] = useState("");
  const [cmdOutput, setCmdOutput] = useState<string[]>([]);
  const outputRef = useRef<HTMLDivElement>(null);

  // Advanced state
  const [advBssid, setAdvBssid] = useState("");
  const [advIface, setAdvIface] = useState("wlan0");
  const [advChannel, setAdvChannel] = useState("6");

  const clearError = () => setError(null);

  const runAttack = useCallback(async (name: RunningAttack, fn: () => Promise<void>) => {
    if (running) return;
    setLoading(true);
    setError(null);
    try {
      await fn();
      setRunning(name);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [running]);

  const stopCurrentAttack = useCallback(async (stopFn?: () => Promise<void>) => {
    setLoading(true);
    setError(null);
    try {
      if (stopFn) await stopFn();
      else await invoke("stop_attack");
      setRunning(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  /* ─── Beacon Handlers ─── */
  const launchBeacon = () => {
    const params: Record<string, unknown> = { interfaceName: beaconIface, mode: beaconMode };
    if (beaconMode === "list") {
      params.ssids = ssidText.split("\n").map(s => s.trim()).filter(Boolean);
    } else if (beaconMode === "random") {
      params.count = parseInt(randomCount, 10) || 50;
    }
    return runAttack("beacon", () => invoke("start_beacon_spam", params));
  };

  /* ─── Deauth Handlers ─── */
  const launchDeauth = () => {
    const params: Record<string, unknown> = { interfaceName: deauthIface };
    if (deauthBssid.trim()) params.targetBssid = deauthBssid.trim();
    return runAttack("deauth", () => invoke("start_mdk4_deauth", params));
  };

  const launchProbe = () =>
    runAttack("probe", () => invoke("start_probe_flood", { interfaceName: deauthIface }));

  /* ─── Portal Handlers ─── */
  const toggleBettercap = async () => {
    setLoading(true);
    setError(null);
    try {
      if (bettercapRunning) {
        await invoke("stop_bettercap_daemon");
        setBettercapRunning(false);
        setCmdOutput(prev => [...prev, ">> Bettercap stopped"]);
      } else {
        await invoke("start_bettercap_daemon", { interfaceName: portalIface });
        setBettercapRunning(true);
        setCmdOutput(prev => [...prev, ">> Bettercap daemon started"]);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const launchPortal = () =>
    runAttack("portal", () =>
      invoke("start_evil_portal", { ssid: portalSsid, channel: parseInt(portalChannel, 10) })
    );

  const launchKarma = () =>
    runAttack("karma", () => invoke("start_karma_attack"));

  const sendBettercapCmd = async () => {
    if (!bettercapCmd.trim()) return;
    setCmdOutput(prev => [...prev, `> ${bettercapCmd}`]);
    try {
      const result = await invoke<string>("bettercap_command", { command: bettercapCmd });
      setCmdOutput(prev => [...prev, result]);
    } catch (e) {
      setCmdOutput(prev => [...prev, `[ERROR] ${String(e)}`]);
    }
    setBettercapCmd("");
    setTimeout(() => outputRef.current?.scrollTo(0, outputRef.current.scrollHeight), 50);
  };

  /* ─── Advanced Handlers ─── */
  const advancedAttacks = [
    {
      id: "channel-switch" as const,
      name: "Channel Switch",
      desc: "Force clients to switch channels via spoofed CSA frames",
      icon: <Shuffle size={20} />,
      needsBssid: true,
      needsChannel: true,
      invoke: async () => { await invoke("start_channel_switch", {
        interfaceName: advIface, targetBssid: advBssid, targetChannel: parseInt(advChannel, 10),
      }); },
    },
    {
      id: "sleep" as const,
      name: "Sleep Attack",
      desc: "Send power-save frames to drain target client batteries",
      icon: <Moon size={20} />,
      needsBssid: true,
      needsChannel: false,
      invoke: async () => { await invoke("start_sleep_attack", { interfaceName: advIface, targetBssid: advBssid }); },
    },
    {
      id: "sae-flood" as const,
      name: "SAE Flood",
      desc: "Flood WPA3 SAE authentication to exhaust AP resources",
      icon: <Sparkles size={20} />,
      needsBssid: true,
      needsChannel: false,
      invoke: async () => { await invoke("start_sae_flood", { interfaceName: advIface, targetBssid: advBssid }); },
    },
    {
      id: "quiet-time" as const,
      name: "Quiet Time",
      desc: "Inject quiet period elements to silence channel activity",
      icon: <Volume2 size={20} />,
      needsBssid: false,
      needsChannel: true,
      invoke: async () => { await invoke("start_quiet_time", { interfaceName: advIface, channel: parseInt(advChannel, 10) }); },
    },
    {
      id: "bad-message" as const,
      name: "Bad Message",
      desc: "Send malformed management frames to trigger AP errors",
      icon: <MessageSquareWarning size={20} />,
      needsBssid: true,
      needsChannel: false,
      invoke: async () => { await invoke("start_bad_message", { interfaceName: advIface, targetBssid: advBssid }); },
    },
  ];

  const stopAdvanced = () => stopCurrentAttack(() => invoke("stop_advanced_attack"));

  const channelOptions = Array.from({ length: 14 }, (_, i) => ({
    value: String(i + 1),
    label: `Channel ${i + 1}`,
  }));

  const tabs = [
    { id: "beacon", label: "Beacon Spam", icon: <Radio size={14} /> },
    { id: "deauth", label: "Deauth", icon: <WifiOff size={14} /> },
    { id: "portal", label: "Evil Portal", icon: <Globe size={14} /> },
    { id: "advanced", label: "Advanced", icon: <Skull size={14} /> },
  ];

  return (
    <div className="h-full flex flex-col animate-in fade-in slide-in-from-bottom-4 duration-500">
      <PageHeader
        icon={<Zap size={28} className="text-destructive" />}
        title="ATTACK"
        subtitle="OFFENSIVE SUITE"
        accent="text-destructive"
      >
        <StatusBadge
          status={running ? "active" : "inactive"}
          label={running ? `${running.toUpperCase()} RUNNING` : "IDLE"}
        />
      </PageHeader>

      <TabBar tabs={tabs} active={tab} onChange={(id) => setTab(id as AttackTab)} />

      {/* Error banner */}
      <AnimatePresence>
        {error && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            className="mb-4 glass-panel rounded-lg border-destructive/50 bg-destructive/10 p-3 flex items-center gap-3 cursor-pointer"
            onClick={clearError}
          >
            <AlertTriangle size={16} className="text-destructive shrink-0" />
            <span className="font-mono text-xs text-destructive">{error}</span>
          </motion.div>
        )}
      </AnimatePresence>

      {/* ═══ BEACON SPAM TAB ═══ */}
      {tab === "beacon" && (
        <motion.div initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} className="space-y-4">
          <GlassCard accent="destructive" className="p-6">
            <h3 className="font-mono text-sm uppercase tracking-widest text-foreground mb-4 flex items-center gap-2">
              <Radio size={16} className="text-destructive" /> Beacon Mode
            </h3>

            <div className="flex gap-3 mb-6">
              {(["list", "random", "rickroll"] as BeaconMode[]).map((m) => (
                <button
                  key={m}
                  onClick={() => setBeaconMode(m)}
                  className={`flex items-center gap-2 px-4 py-2 rounded font-mono text-xs uppercase tracking-widest border transition-all ${
                    beaconMode === m
                      ? "bg-destructive/15 border-destructive/40 text-destructive"
                      : "bg-black/20 border-border/40 text-muted-foreground hover:text-foreground"
                  }`}
                >
                  {m === "list" && <List size={14} />}
                  {m === "random" && <Shuffle size={14} />}
                  {m === "rickroll" && <Music size={14} />}
                  {m}
                </button>
              ))}
            </div>

            <AnimatePresence mode="wait">
              {beaconMode === "list" && (
                <motion.div key="list" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
                  <label className="text-xs font-mono uppercase tracking-widest text-muted-foreground mb-2 block">
                    SSIDs (one per line)
                  </label>
                  <textarea
                    value={ssidText}
                    onChange={(e) => setSsidText(e.target.value)}
                    rows={5}
                    className="w-full bg-black/40 border border-border/60 rounded px-4 py-2 font-mono text-sm text-foreground focus:border-destructive focus:outline-none transition-colors resize-none"
                    placeholder="Enter SSIDs, one per line..."
                  />
                </motion.div>
              )}
              {beaconMode === "random" && (
                <motion.div key="random" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
                  <InputField label="Number of random SSIDs" value={randomCount} onChange={setRandomCount} type="number" placeholder="50" />
                </motion.div>
              )}
              {beaconMode === "rickroll" && (
                <motion.div key="rickroll" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
                  className="text-center py-4"
                >
                  <Music size={32} className="text-destructive mx-auto mb-2 opacity-60" />
                  <p className="font-mono text-xs text-muted-foreground">
                    Broadcasts Rick Astley lyrics as SSIDs
                  </p>
                </motion.div>
              )}
            </AnimatePresence>

            <div className="mt-6">
              <InputField label="Interface" value={beaconIface} onChange={setBeaconIface} placeholder="wlan0" />
            </div>

            <div className="mt-6 flex items-center gap-4">
              {running === "beacon" ? (
                <ActionButton variant="ghost" size="lg" onClick={() => stopCurrentAttack()} loading={loading} className="flex-1">
                  <Square size={16} /> STOP
                </ActionButton>
              ) : (
                <ActionButton variant="destructive" size="lg" onClick={launchBeacon} loading={loading} disabled={!!running} className="flex-1">
                  <Play size={16} /> LAUNCH
                </ActionButton>
              )}
              <StatusBadge status={running === "beacon" ? "active" : "inactive"} label={running === "beacon" ? "TRANSMITTING" : "IDLE"} />
            </div>
          </GlassCard>
        </motion.div>
      )}

      {/* ═══ DEAUTH TAB ═══ */}
      {tab === "deauth" && (
        <motion.div initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} className="space-y-4">
          {/* Warning banner */}
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            className="glass-panel rounded-lg border-radar-yellow/30 bg-radar-yellow/5 p-4 flex items-start gap-3"
          >
            <AlertTriangle size={18} className="text-radar-yellow shrink-0 mt-0.5" />
            <div>
              <p className="font-mono text-xs text-radar-yellow uppercase tracking-widest font-bold">Legal Warning</p>
              <p className="font-mono text-[11px] text-radar-yellow/70 mt-1">
                Deauthentication attacks may be illegal without explicit authorization. Use only on networks you own or have written permission to test.
              </p>
            </div>
          </motion.div>

          <GlassCard accent="destructive" className="p-6 space-y-4">
            <InputField label="Target BSSID (blank = broadcast)" value={deauthBssid} onChange={setDeauthBssid} placeholder="AA:BB:CC:DD:EE:FF" />
            <InputField label="Interface" value={deauthIface} onChange={setDeauthIface} placeholder="wlan0" />

            <div className="flex gap-3 pt-2">
              {running === "probe" ? (
                <ActionButton variant="ghost" size="lg" onClick={() => stopCurrentAttack()} loading={loading} className="flex-1">
                  <Square size={16} /> STOP PROBE
                </ActionButton>
              ) : (
                <ActionButton variant="destructive" size="lg" onClick={launchProbe} loading={loading} disabled={!!running} className="flex-1">
                  <Wifi size={16} /> PROBE FLOOD
                </ActionButton>
              )}
              {running === "deauth" ? (
                <ActionButton variant="ghost" size="lg" onClick={() => stopCurrentAttack()} loading={loading} className="flex-1">
                  <Square size={16} /> STOP DEAUTH
                </ActionButton>
              ) : (
                <ActionButton variant="destructive" size="lg" onClick={launchDeauth} loading={loading} disabled={!!running} className="flex-1">
                  <WifiOff size={16} /> DEAUTH
                </ActionButton>
              )}
            </div>

            <div className="pt-2">
              <StatusBadge
                status={running === "deauth" || running === "probe" ? "active" : "inactive"}
                label={running === "deauth" ? "DEAUTH ACTIVE" : running === "probe" ? "PROBE FLOOD ACTIVE" : "IDLE"}
              />
            </div>
          </GlassCard>
        </motion.div>
      )}

      {/* ═══ EVIL PORTAL TAB ═══ */}
      {tab === "portal" && (
        <motion.div initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            {/* Portal config */}
            <GlassCard accent="destructive" className="p-6 space-y-4">
              <h3 className="font-mono text-sm uppercase tracking-widest text-foreground flex items-center gap-2">
                <Globe size={16} className="text-destructive" /> Portal Config
              </h3>
              <InputField label="SSID" value={portalSsid} onChange={setPortalSsid} placeholder="FreeWiFi" />
              <SelectField label="Channel" value={portalChannel} onChange={setPortalChannel} options={channelOptions} />
              <InputField label="Interface" value={portalIface} onChange={setPortalIface} placeholder="wlan0" />

              <div className="pt-2 space-y-3">
                <ActionButton
                  variant={bettercapRunning ? "ghost" : "destructive"}
                  size="md"
                  onClick={toggleBettercap}
                  loading={loading}
                  className="w-full justify-center"
                >
                  {bettercapRunning ? <><Square size={14} /> Stop Bettercap</> : <><Play size={14} /> Start Bettercap</>}
                </ActionButton>

                <div className="flex gap-2">
                  <ActionButton
                    variant="destructive" size="md" onClick={launchPortal}
                    loading={loading && !running} disabled={!!running || !bettercapRunning}
                    className="flex-1 justify-center"
                  >
                    <Shield size={14} /> Evil Portal
                  </ActionButton>
                  <ActionButton
                    variant="destructive" size="md" onClick={launchKarma}
                    loading={loading && !running} disabled={!!running || !bettercapRunning}
                    className="flex-1 justify-center"
                  >
                    <Sparkles size={14} /> Karma
                  </ActionButton>
                </div>

                {running && (running === "portal" || running === "karma") && (
                  <ActionButton variant="ghost" size="md" onClick={() => stopCurrentAttack()} loading={loading} className="w-full justify-center">
                    <Square size={14} /> Stop Attack
                  </ActionButton>
                )}
              </div>
            </GlassCard>

            {/* Bettercap terminal */}
            <GlassCard accent="none" className="p-6 flex flex-col">
              <h3 className="font-mono text-sm uppercase tracking-widest text-foreground flex items-center gap-2 mb-4">
                <Terminal size={16} className="text-primary" /> Bettercap Console
              </h3>

              <div
                ref={outputRef}
                className="flex-1 bg-black/60 rounded border border-border/40 p-3 overflow-y-auto font-mono text-[11px] text-radar-green/80 min-h-[200px] max-h-[340px] space-y-0.5"
              >
                {cmdOutput.length === 0 ? (
                  <span className="text-muted-foreground italic">No output yet...</span>
                ) : (
                  cmdOutput.map((line, i) => (
                    <div key={i} className={line.startsWith("[ERROR]") ? "text-destructive" : line.startsWith(">>") ? "text-primary" : ""}>
                      {line}
                    </div>
                  ))
                )}
              </div>

              <div className="flex gap-2 mt-3">
                <input
                  value={bettercapCmd}
                  onChange={(e) => setBettercapCmd(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && sendBettercapCmd()}
                  placeholder={bettercapRunning ? "wifi.show" : "Start bettercap first..."}
                  disabled={!bettercapRunning}
                  className="flex-1 bg-black/40 border border-border/60 rounded px-3 py-2 font-mono text-xs text-foreground focus:border-primary focus:outline-none transition-colors disabled:opacity-40"
                />
                <ActionButton variant="primary" size="sm" onClick={sendBettercapCmd} disabled={!bettercapRunning || !bettercapCmd.trim()}>
                  <Send size={14} />
                </ActionButton>
              </div>
            </GlassCard>
          </div>
        </motion.div>
      )}

      {/* ═══ ADVANCED TAB ═══ */}
      {tab === "advanced" && (
        <motion.div initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} className="space-y-4">
          {/* Shared inputs */}
          <GlassCard accent="none" className="p-4">
            <div className="grid grid-cols-3 gap-4">
              <InputField label="Interface" value={advIface} onChange={setAdvIface} placeholder="wlan0" />
              <InputField label="Target BSSID" value={advBssid} onChange={setAdvBssid} placeholder="AA:BB:CC:DD:EE:FF" />
              <SelectField label="Channel" value={advChannel} onChange={setAdvChannel} options={channelOptions} />
            </div>
          </GlassCard>

          {/* Attack grid */}
          <div className="grid grid-cols-2 gap-4">
            {advancedAttacks.map((atk) => {
              const isRunning = running === atk.id;
              const missingBssid = atk.needsBssid && !advBssid.trim();
              const disabled = (!!running && !isRunning) || (!isRunning && missingBssid);

              return (
                <GlassCard key={atk.id} accent="destructive" className="p-5">
                  <div className="flex items-start justify-between mb-3">
                    <div className="flex items-center gap-3">
                      <div className="w-9 h-9 rounded-lg bg-destructive/10 border border-destructive/20 flex items-center justify-center text-destructive">
                        {atk.icon}
                      </div>
                      <div>
                        <h4 className="font-mono text-sm font-bold text-foreground uppercase tracking-wider">{atk.name}</h4>
                        <p className="text-[11px] text-muted-foreground mt-0.5">{atk.desc}</p>
                      </div>
                    </div>
                    <StatusBadge status={isRunning ? "active" : "inactive"} pulse={isRunning} />
                  </div>

                  <div className="flex items-center gap-2 text-[10px] font-mono text-muted-foreground mb-3">
                    {atk.needsBssid && (
                      <span className={`px-2 py-0.5 rounded border ${advBssid.trim() ? "border-radar-green/30 text-radar-green" : "border-destructive/30 text-destructive"}`}>
                        BSSID {advBssid.trim() ? "✓" : "Required"}
                      </span>
                    )}
                    {atk.needsChannel && (
                      <span className="px-2 py-0.5 rounded border border-primary/30 text-primary">CH {advChannel}</span>
                    )}
                  </div>

                  {isRunning ? (
                    <ActionButton variant="ghost" size="sm" onClick={stopAdvanced} loading={loading} className="w-full justify-center">
                      <Square size={14} /> STOP
                    </ActionButton>
                  ) : (
                    <ActionButton
                      variant="destructive" size="sm"
                      onClick={() => runAttack(atk.id, atk.invoke)}
                      loading={loading} disabled={disabled}
                      className="w-full justify-center"
                    >
                      <Play size={14} /> LAUNCH
                    </ActionButton>
                  )}
                </GlassCard>
              );
            })}
          </div>
        </motion.div>
      )}
    </div>
  );
}
