import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Wrench, Shuffle, RotateCcw, Plus, Trash2, Sparkles,
  Save, Wifi, WifiOff, MapPin, MapPinOff, Download,
  FileDown, CheckSquare, Square, List, XCircle, CheckCircle2,
} from "lucide-react";
import type { MacSpoofResult, SsidList, SavedAp, GpsLocation, WardriveEntry } from "../types/capture";
import {
  PageHeader, TabBar, GlassCard, ActionButton, InputField, DataTable, EmptyState, SignalBar,
} from "../components/ui/shared";

/* ─── Toast ─── */
function Toast({ message, type, onDismiss }: { message: string; type: "success" | "error"; onDismiss: () => void }) {
  useEffect(() => {
    const t = setTimeout(onDismiss, 4000);
    return () => clearTimeout(t);
  }, [onDismiss]);

  return (
    <div
      className={`fixed bottom-6 right-6 z-50 flex items-center gap-3 px-5 py-3 rounded-lg border font-mono text-sm shadow-2xl backdrop-blur-xl animate-in fade-in slide-in-from-bottom-4 duration-300 ${
        type === "success"
          ? "bg-radar-green/10 border-radar-green/40 text-radar-green"
          : "bg-destructive/10 border-destructive/40 text-destructive"
      }`}
    >
      {type === "success" ? <CheckCircle2 className="w-4 h-4" /> : <XCircle className="w-4 h-4" />}
      {message}
    </div>
  );
}

/* ─── helpers ─── */
function errMsg(err: unknown): string {
  if (typeof err === "string") return err;
  if (err && typeof err === "object" && "message" in err) return String((err as { message: string }).message);
  return JSON.stringify(err);
}

/* ───────────────────────── MAC Spoof Tab ───────────────────────── */
function MacSpoofTab() {
  const [iface, setIface] = useState("wlan0");
  const [newMac, setNewMac] = useState("");
  const [loading, setLoading] = useState<"spoof" | "restore" | null>(null);
  const [result, setResult] = useState<MacSpoofResult | null>(null);
  const [toast, setToast] = useState<{ message: string; type: "success" | "error" } | null>(null);

  const handleSpoof = async () => {
    setLoading("spoof");
    setResult(null);
    try {
      const r = await invoke<MacSpoofResult>("spoof_mac", {
        interfaceName: iface,
        newMac: newMac.trim() || null,
      });
      setResult(r);
      setToast({ message: r.success ? "MAC address spoofed" : r.message, type: r.success ? "success" : "error" });
    } catch (err) {
      setToast({ message: errMsg(err), type: "error" });
    } finally {
      setLoading(null);
    }
  };

  const handleRestore = async () => {
    setLoading("restore");
    setResult(null);
    try {
      const r = await invoke<MacSpoofResult>("restore_mac", { interfaceName: iface });
      setResult(r);
      setToast({ message: r.success ? "MAC address restored" : r.message, type: r.success ? "success" : "error" });
    } catch (err) {
      setToast({ message: errMsg(err), type: "error" });
    } finally {
      setLoading(null);
    }
  };

  return (
    <div className="space-y-6">
      <GlassCard className="p-6">
        <h3 className="font-mono text-sm uppercase tracking-widest text-primary mb-6">MAC Address Spoofing</h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
          <InputField label="Interface" value={iface} onChange={setIface} placeholder="wlan0" />
          <InputField label="New MAC (empty = random)" value={newMac} onChange={setNewMac} placeholder="AA:BB:CC:DD:EE:FF" />
        </div>
        <div className="flex gap-3">
          <ActionButton onClick={handleSpoof} loading={loading === "spoof"} disabled={loading !== null}>
            <Shuffle className="w-4 h-4" /> Randomize MAC
          </ActionButton>
          <ActionButton onClick={handleRestore} variant="ghost" loading={loading === "restore"} disabled={loading !== null}>
            <RotateCcw className="w-4 h-4" /> Restore Original
          </ActionButton>
        </div>
      </GlassCard>

      {result && (
        <GlassCard accent={result.success ? "green" : "none"} className="p-6">
          <h4 className="font-mono text-xs uppercase tracking-widest text-muted-foreground mb-4">Result</h4>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4 font-mono text-sm">
            <div>
              <span className="text-muted-foreground text-xs block mb-1">Original MAC</span>
              <span className="text-foreground">{result.original_mac}</span>
            </div>
            <div>
              <span className="text-muted-foreground text-xs block mb-1">New MAC</span>
              <span className="text-primary">{result.new_mac}</span>
            </div>
            <div>
              <span className="text-muted-foreground text-xs block mb-1">Vendor</span>
              <span className="text-foreground">{result.vendor ?? "Unknown"}</span>
            </div>
          </div>
          <div className="mt-4">
            <span
              className={`inline-flex items-center gap-2 px-3 py-1 rounded text-xs font-mono uppercase tracking-widest ${
                result.success
                  ? "bg-radar-green/10 text-radar-green border border-radar-green/30"
                  : "bg-destructive/10 text-destructive border border-destructive/30"
              }`}
            >
              {result.success ? <CheckCircle2 className="w-3 h-3" /> : <XCircle className="w-3 h-3" />}
              {result.message}
            </span>
          </div>
        </GlassCard>
      )}

      {toast && <Toast message={toast.message} type={toast.type} onDismiss={() => setToast(null)} />}
    </div>
  );
}

/* ───────────────────────── SSID Manager Tab ───────────────────────── */
function SsidManagerTab() {
  const [listNames, setListNames] = useState<string[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [ssidText, setSsidText] = useState("");
  const [newName, setNewName] = useState("");
  const [genCount, setGenCount] = useState("20");
  const [loading, setLoading] = useState(false);
  const [toast, setToast] = useState<{ message: string; type: "success" | "error" } | null>(null);

  const fetchLists = useCallback(async () => {
    try {
      const names = await invoke<string[]>("list_ssid_lists");
      setListNames(names);
    } catch (err) {
      setToast({ message: errMsg(err), type: "error" });
    }
  }, []);

  useEffect(() => { fetchLists(); }, [fetchLists]);

  const handleSelectList = async (name: string) => {
    setSelected(name);
    try {
      const list = await invoke<SsidList | null>("get_ssid_list", { name });
      setSsidText(list ? list.ssids.join("\n") : "");
    } catch (err) {
      setToast({ message: errMsg(err), type: "error" });
    }
  };

  const handleSave = async () => {
    if (!selected) return;
    setLoading(true);
    try {
      const ssids = ssidText.split("\n").map((s) => s.trim()).filter(Boolean);
      await invoke("save_ssid_list", { name: selected, ssids });
      setToast({ message: `Saved "${selected}"`, type: "success" });
      await fetchLists();
    } catch (err) {
      setToast({ message: errMsg(err), type: "error" });
    } finally {
      setLoading(false);
    }
  };

  const handleDelete = async () => {
    if (!selected) return;
    setLoading(true);
    try {
      await invoke("delete_ssid_list", { name: selected });
      setSelected(null);
      setSsidText("");
      setToast({ message: `Deleted "${selected}"`, type: "success" });
      await fetchLists();
    } catch (err) {
      setToast({ message: errMsg(err), type: "error" });
    } finally {
      setLoading(false);
    }
  };

  const handleCreate = async () => {
    const name = newName.trim();
    if (!name) return;
    setLoading(true);
    try {
      await invoke("save_ssid_list", { name, ssids: [] });
      setNewName("");
      await fetchLists();
      setSelected(name);
      setSsidText("");
      setToast({ message: `Created "${name}"`, type: "success" });
    } catch (err) {
      setToast({ message: errMsg(err), type: "error" });
    } finally {
      setLoading(false);
    }
  };

  const handleGenerate = async () => {
    setLoading(true);
    try {
      const count = Math.max(1, Math.min(100, parseInt(genCount) || 20));
      const generated = await invoke<string[]>("generate_random_ssids", { count, maxLen: 16 });
      setSsidText((prev) => (prev ? prev + "\n" : "") + generated.join("\n"));
      setToast({ message: `Generated ${generated.length} SSIDs`, type: "success" });
    } catch (err) {
      setToast({ message: errMsg(err), type: "error" });
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
      {/* Left: list browser */}
      <GlassCard className="p-5 lg:col-span-1">
        <h3 className="font-mono text-sm uppercase tracking-widest text-primary mb-4">SSID Lists</h3>

        <div className="flex gap-2 mb-4">
          <input
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            placeholder="New list name…"
            className="flex-1 bg-black/40 border border-border/60 rounded px-3 py-1.5 text-sm font-mono text-foreground focus:border-primary focus:outline-none transition-colors"
            onKeyDown={(e) => e.key === "Enter" && handleCreate()}
          />
          <ActionButton size="sm" onClick={handleCreate} disabled={!newName.trim() || loading}>
            <Plus className="w-3.5 h-3.5" />
          </ActionButton>
        </div>

        <div className="space-y-1 max-h-[400px] overflow-auto">
          {listNames.length === 0 ? (
            <p className="text-xs text-muted-foreground italic py-4 text-center">No SSID lists yet</p>
          ) : (
            listNames.map((name) => (
              <button
                key={name}
                onClick={() => handleSelectList(name)}
                className={`w-full text-left px-3 py-2 rounded text-sm font-mono transition-colors ${
                  selected === name
                    ? "bg-primary/15 text-primary border border-primary/20"
                    : "text-muted-foreground hover:bg-muted/50 hover:text-foreground"
                }`}
              >
                <List className="w-3.5 h-3.5 inline mr-2" />
                {name}
              </button>
            ))
          )}
        </div>
      </GlassCard>

      {/* Right: editor */}
      <GlassCard className="p-5 lg:col-span-2">
        {selected ? (
          <>
            <h3 className="font-mono text-sm uppercase tracking-widest text-primary mb-4">
              Editing: {selected}
            </h3>
            <textarea
              value={ssidText}
              onChange={(e) => setSsidText(e.target.value)}
              placeholder="One SSID per line…"
              rows={14}
              className="w-full bg-black/40 border border-border/60 rounded px-4 py-3 text-sm font-mono text-foreground focus:border-primary focus:outline-none transition-colors resize-none mb-4"
            />
            <div className="flex flex-wrap gap-3 items-end">
              <ActionButton onClick={handleSave} loading={loading} disabled={loading}>
                <Save className="w-4 h-4" /> Save
              </ActionButton>
              <ActionButton onClick={handleDelete} variant="destructive" disabled={loading}>
                <Trash2 className="w-4 h-4" /> Delete
              </ActionButton>
              <div className="flex items-end gap-2 ml-auto">
                <InputField
                  label="Count"
                  value={genCount}
                  onChange={setGenCount}
                  className="w-20"
                  type="number"
                />
                <ActionButton onClick={handleGenerate} variant="ghost" loading={loading} disabled={loading}>
                  <Sparkles className="w-4 h-4" /> Generate
                </ActionButton>
              </div>
            </div>
          </>
        ) : (
          <EmptyState icon={<List className="w-12 h-12" />} title="No list selected" description="Select or create an SSID list to edit" />
        )}
      </GlassCard>

      {toast && <Toast message={toast.message} type={toast.type} onDismiss={() => setToast(null)} />}
    </div>
  );
}

/* ───────────────────────── AP Manager Tab ───────────────────────── */
function ApManagerTab() {
  const [aps, setAps] = useState<SavedAp[]>([]);
  const [loading, setLoading] = useState(false);
  const [toast, setToast] = useState<{ message: string; type: "success" | "error" } | null>(null);

  const fetchAps = useCallback(async () => {
    try {
      const data = await invoke<SavedAp[]>("load_saved_aps");
      setAps(data);
    } catch (err) {
      setToast({ message: errMsg(err), type: "error" });
    }
  }, []);

  useEffect(() => { fetchAps(); }, [fetchAps]);

  const selectedCount = aps.filter((a) => a.selected).length;

  const handleToggle = async (bssid: string) => {
    const ap = aps.find((a) => a.bssid === bssid);
    if (!ap) return;
    try {
      await invoke("select_target_aps", { bssids: [bssid], selected: !ap.selected });
      setAps((prev) => prev.map((a) => (a.bssid === bssid ? { ...a, selected: !a.selected } : a)));
    } catch (err) {
      setToast({ message: errMsg(err), type: "error" });
    }
  };

  const handleClear = async () => {
    setLoading(true);
    try {
      await invoke("clear_saved_aps");
      setAps([]);
      setToast({ message: "Cleared all saved APs", type: "success" });
    } catch (err) {
      setToast({ message: errMsg(err), type: "error" });
    } finally {
      setLoading(false);
    }
  };

  type Row = Record<string, unknown>;

  const columns = [
    {
      key: "selected",
      label: "Sel",
      align: "center" as const,
      render: (row: Row) => {
        const ap = row as unknown as SavedAp;
        return (
          <button onClick={(e) => { e.stopPropagation(); handleToggle(ap.bssid); }} className="text-primary hover:text-primary/80 transition-colors">
            {ap.selected ? <CheckSquare className="w-4 h-4" /> : <Square className="w-4 h-4 text-muted-foreground" />}
          </button>
        );
      },
    },
    { key: "bssid", label: "BSSID" },
    { key: "ssid", label: "SSID", render: (row: Row) => { const ap = row as unknown as SavedAp; return <span className="text-primary">{ap.ssid || <span className="italic text-muted-foreground">Hidden</span>}</span>; } },
    { key: "channel", label: "CH", align: "center" as const },
    { key: "rssi", label: "RSSI", render: (row: Row) => <SignalBar rssi={(row as unknown as SavedAp).rssi} /> },
    { key: "encryption", label: "Encryption", render: (row: Row) => <span className="text-xs">{(row as unknown as SavedAp).encryption ?? "Open"}</span> },
    { key: "vendor", label: "Vendor", render: (row: Row) => <span className="text-xs text-muted-foreground">{(row as unknown as SavedAp).vendor ?? "—"}</span> },
  ];

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4">
          <span className="font-mono text-xs text-muted-foreground uppercase tracking-widest">
            {aps.length} APs saved · <span className="text-primary">{selectedCount} selected</span>
          </span>
          <span className="text-xs text-muted-foreground italic">Selected APs are used as targets in Attack page</span>
        </div>
        <ActionButton onClick={handleClear} variant="destructive" size="sm" loading={loading} disabled={loading || aps.length === 0}>
          <Trash2 className="w-3.5 h-3.5" /> Clear All
        </ActionButton>
      </div>

      <DataTable
        columns={columns}
        data={aps as unknown as Record<string, unknown>[]}
        keyField="bssid"
        emptyMessage="No saved APs. Run a scan from Dashboard to populate."
        maxHeight="500px"
        onRowClick={(item) => handleToggle(String(item.bssid))}
      />

      {toast && <Toast message={toast.message} type={toast.type} onDismiss={() => setToast(null)} />}
    </div>
  );
}

/* ───────────────────────── WiFi Tab ───────────────────────── */
function WifiTab() {
  const [iface, setIface] = useState("wlan0");
  const [ssid, setSsid] = useState("");
  const [password, setPassword] = useState("");
  const [loading, setLoading] = useState<"connect" | "disconnect" | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [toast, setToast] = useState<{ message: string; type: "success" | "error" } | null>(null);

  const handleConnect = async () => {
    if (!ssid.trim()) return;
    setLoading("connect");
    setStatus(null);
    try {
      const msg = await invoke<string>("join_wifi", {
        interfaceName: iface,
        ssid: ssid.trim(),
        password: password,
      });
      setStatus(msg);
      setToast({ message: "Connected successfully", type: "success" });
    } catch (err) {
      const msg = errMsg(err);
      setStatus(msg);
      setToast({ message: msg, type: "error" });
    } finally {
      setLoading(null);
    }
  };

  const handleDisconnect = async () => {
    setLoading("disconnect");
    setStatus(null);
    try {
      const msg = await invoke<string>("disconnect_wifi", { interfaceName: iface });
      setStatus(msg);
      setToast({ message: "Disconnected", type: "success" });
    } catch (err) {
      const msg = errMsg(err);
      setStatus(msg);
      setToast({ message: msg, type: "error" });
    } finally {
      setLoading(null);
    }
  };

  return (
    <div className="space-y-6">
      <GlassCard className="p-6">
        <h3 className="font-mono text-sm uppercase tracking-widest text-primary mb-6">WiFi Connection</h3>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
          <InputField label="Interface" value={iface} onChange={setIface} placeholder="wlan0" />
          <InputField label="SSID" value={ssid} onChange={setSsid} placeholder="Network name" />
          <InputField label="Password" value={password} onChange={setPassword} placeholder="••••••••" type="password" />
        </div>
        <div className="flex gap-3">
          <ActionButton onClick={handleConnect} loading={loading === "connect"} disabled={loading !== null || !ssid.trim()}>
            <Wifi className="w-4 h-4" /> Connect
          </ActionButton>
          <ActionButton onClick={handleDisconnect} variant="ghost" loading={loading === "disconnect"} disabled={loading !== null}>
            <WifiOff className="w-4 h-4" /> Disconnect
          </ActionButton>
        </div>
      </GlassCard>

      {status && (
        <GlassCard className="p-5">
          <h4 className="font-mono text-xs uppercase tracking-widest text-muted-foreground mb-2">Status</h4>
          <p className="font-mono text-sm text-foreground">{status}</p>
        </GlassCard>
      )}

      {toast && <Toast message={toast.message} type={toast.type} onDismiss={() => setToast(null)} />}
    </div>
  );
}

/* ───────────────────────── Wardrive Tab ───────────────────────── */
function WardriveTab() {
  const [gps, setGps] = useState<GpsLocation | null>(null);
  const [gpsChecked, setGpsChecked] = useState(false);
  const [loading, setLoading] = useState<"gps" | "csv" | "kml" | null>(null);
  const [toast, setToast] = useState<{ message: string; type: "success" | "error" } | null>(null);

  const fixModeLabel = (mode: number) => {
    if (mode === 3) return "3D Fix";
    if (mode === 2) return "2D Fix";
    return "No Fix";
  };

  const handleCheckGps = async () => {
    setLoading("gps");
    try {
      const loc = await invoke<GpsLocation | null>("get_gps_location");
      setGps(loc);
      setGpsChecked(true);
      if (loc) {
        setToast({ message: `GPS: ${fixModeLabel(loc.fix_mode)}`, type: "success" });
      } else {
        setToast({ message: "GPS not available", type: "error" });
      }
    } catch (err) {
      setGps(null);
      setGpsChecked(true);
      setToast({ message: errMsg(err), type: "error" });
    } finally {
      setLoading(null);
    }
  };

  const handleExport = async (format: "csv" | "kml") => {
    setLoading(format);
    try {
      const entries: WardriveEntry[] = [];
      const ext = format === "csv" ? "csv" : "kml";
      const outputPath = `wardrive_export.${ext}`;
      const cmd = format === "csv" ? "export_wardrive_csv" : "export_wardrive_kml";
      const count = await invoke<number>(cmd, { entries, outputPath });
      setToast({ message: `Exported ${count} entries to ${outputPath}`, type: "success" });
    } catch (err) {
      setToast({ message: errMsg(err), type: "error" });
    } finally {
      setLoading(null);
    }
  };

  return (
    <div className="space-y-6">
      <GlassCard className="p-6">
        <div className="flex items-center justify-between mb-6">
          <h3 className="font-mono text-sm uppercase tracking-widest text-primary">GPS Status</h3>
          <ActionButton onClick={handleCheckGps} loading={loading === "gps"} disabled={loading !== null} size="sm">
            <MapPin className="w-4 h-4" /> Check GPS
          </ActionButton>
        </div>

        {gpsChecked && gps ? (
          <div className="space-y-4">
            <div className="flex items-center gap-3 mb-4">
              <MapPin className="w-5 h-5 text-radar-green" />
              <span className="font-mono text-sm text-radar-green uppercase tracking-widest">
                {fixModeLabel(gps.fix_mode)}
              </span>
            </div>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4 font-mono text-sm">
              <div>
                <span className="text-muted-foreground text-xs block mb-1">Latitude</span>
                <span className="text-foreground">{gps.latitude.toFixed(6)}</span>
              </div>
              <div>
                <span className="text-muted-foreground text-xs block mb-1">Longitude</span>
                <span className="text-foreground">{gps.longitude.toFixed(6)}</span>
              </div>
              <div>
                <span className="text-muted-foreground text-xs block mb-1">Altitude</span>
                <span className="text-foreground">{gps.altitude.toFixed(1)} m</span>
              </div>
              <div>
                <span className="text-muted-foreground text-xs block mb-1">Speed</span>
                <span className="text-foreground">{gps.speed.toFixed(1)} m/s</span>
              </div>
            </div>
          </div>
        ) : gpsChecked ? (
          <div className="flex items-center gap-3 py-4">
            <MapPinOff className="w-5 h-5 text-muted-foreground" />
            <span className="font-mono text-sm text-muted-foreground">
              GPS not available. Install <span className="text-primary">gpsd</span> for location data.
            </span>
          </div>
        ) : (
          <p className="text-xs text-muted-foreground italic py-4">Click "Check GPS" to query location</p>
        )}
      </GlassCard>

      <GlassCard className="p-6">
        <h3 className="font-mono text-sm uppercase tracking-widest text-primary mb-6">Export Wardrive Data</h3>
        <div className="flex gap-3">
          <ActionButton onClick={() => handleExport("csv")} variant="ghost" loading={loading === "csv"} disabled={loading !== null}>
            <Download className="w-4 h-4" /> Export WiGLE CSV
          </ActionButton>
          <ActionButton onClick={() => handleExport("kml")} variant="ghost" loading={loading === "kml"} disabled={loading !== null}>
            <FileDown className="w-4 h-4" /> Export KML
          </ActionButton>
        </div>
      </GlassCard>

      {toast && <Toast message={toast.message} type={toast.type} onDismiss={() => setToast(null)} />}
    </div>
  );
}

/* ───────────────────────── Main Tools Page ───────────────────────── */
const TABS = [
  { id: "mac", label: "MAC Spoof" },
  { id: "ssid", label: "SSID Manager" },
  { id: "ap", label: "AP Manager" },
  { id: "wifi", label: "WiFi" },
  { id: "wardrive", label: "Wardrive" },
];

export function Tools() {
  const [activeTab, setActiveTab] = useState("mac");

  return (
    <div className="h-full flex flex-col animate-in fade-in slide-in-from-bottom-4 duration-500">
      <PageHeader
        icon={<Wrench className="w-8 h-8 text-primary" />}
        title="TOOLS"
        subtitle="UTILITY ARSENAL"
      />

      <TabBar tabs={TABS} active={activeTab} onChange={setActiveTab} />

      <div className="flex-1 min-h-0 overflow-auto">
        {activeTab === "mac" && <MacSpoofTab />}
        {activeTab === "ssid" && <SsidManagerTab />}
        {activeTab === "ap" && <ApManagerTab />}
        {activeTab === "wifi" && <WifiTab />}
        {activeTab === "wardrive" && <WardriveTab />}
      </div>
    </div>
  );
}
