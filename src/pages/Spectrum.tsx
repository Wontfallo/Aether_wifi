import { Activity, Gauge, Play, Radio, Signal, Square, Wifi } from "lucide-react";
import ReactECharts from "echarts-for-react";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import { useBeaconCapture } from "../hooks/useBeaconCapture";
import type { BeaconFrame, NetworkInterface } from "../types/capture";
import {
  DataTable,
  EmptyState,
  GlassCard,
  InputField,
  SelectField,
  SignalBar,
  StatusBadge,
} from "../components/ui/shared";

type BandKey = "2.4" | "5" | "6";

type NetworkRow = BeaconFrame & {
  display_name: string;
  age_label: string;
  band_label: string;
  [key: string]: unknown;
};

type ChannelSummary = {
  channel: number;
  frequency: number;
  score: number;
  networkCount: number;
  strongestRssi: number | null;
};

type BandConfig = {
  label: string;
  subtitle: string;
  frequencyRange: [number, number];
  channels: number[];
  envelopeWidthMhz: number;
  axisTickIntervalMhz: number;
};

const FLOOR_DBM = -100;
const PALETTE = ["#00E5FF", "#7CFF5B", "#FFB347", "#6F8CFF", "#FF6B9A", "#D77CFF", "#FFE066"];

const BAND_CONFIG: Record<BandKey, BandConfig> = {
  "2.4": {
    label: "2.4 GHz",
    subtitle: "ISM band · overlap-sensitive",
    frequencyRange: [2400, 2490],
    channels: Array.from({ length: 14 }, (_, index) => index + 1),
    envelopeWidthMhz: 11,
    axisTickIntervalMhz: 5,
  },
  "5": {
    label: "5 GHz",
    subtitle: "UNII bands · high channel reuse",
    frequencyRange: [5150, 5895],
    channels: [36, 40, 44, 48, 52, 56, 60, 64, 100, 104, 108, 112, 116, 120, 124, 128, 132, 136, 140, 144, 149, 153, 157, 161, 165],
    envelopeWidthMhz: 20,
    axisTickIntervalMhz: 20,
  },
  "6": {
    label: "6 GHz",
    subtitle: "6E/7 spectrum preview",
    frequencyRange: [5925, 7125],
    channels: Array.from({ length: 59 }, (_, index) => 1 + index * 4),
    envelopeWidthMhz: 20,
    axisTickIntervalMhz: 20,
  },
};

function toErrorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  try {
    return JSON.stringify(error);
  } catch {
    return "Unknown error";
  }
}

function hashColor(seed: string): string {
  const total = seed.split("").reduce((sum, character) => sum + character.charCodeAt(0), 0);
  return PALETTE[total % PALETTE.length];
}

function hexToRgba(hex: string, alpha: number): string {
  const normalized = hex.replace("#", "");
  const value = Number.parseInt(normalized, 16);
  const r = (value >> 16) & 255;
  const g = (value >> 8) & 255;
  const b = value & 255;
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

function bandFromBeacon(beacon: BeaconFrame): BandKey | null {
  if (beacon.frequency_mhz >= 2400 && beacon.frequency_mhz <= 2490) return "2.4";
  if (beacon.frequency_mhz >= 5000 && beacon.frequency_mhz <= 5895) return "5";
  if (beacon.frequency_mhz >= 5925 && beacon.frequency_mhz <= 7125) return "6";
  return null;
}

function channelToFrequency(channel: number): number {
  if (channel >= 1 && channel <= 13) return 2407 + channel * 5;
  if (channel === 14) return 2484;
  if (channel >= 36 && channel <= 196) return 5000 + channel * 5;
  if (channel >= 1) return 5950 + channel * 5;
  return 0;
}

function estimateDistanceMeters(rssi: number, freqMhz: number): string {
  const fspl = 27.55 - (20 * Math.log10(freqMhz || 2412)) + Math.abs(rssi);
  return Math.pow(10, fspl / 20).toFixed(1);
}

function formatAge(timestampMs: number): string {
  const seconds = Math.max(0, Math.floor((Date.now() - timestampMs) / 1000));
  if (seconds < 5) return "now";
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  return `${Math.floor(seconds / 3600)}h`;
}

function buildEnvelope(centerFreq: number, widthMhz: number, peakRssi: number): [number, number][] {
  const points: [number, number][] = [];
  const steps = 24;

  for (let index = 0; index <= steps; index += 1) {
    const x = centerFreq - widthMhz + (index / steps) * widthMhz * 2;
    const normalizedDistance = Math.abs(x - centerFreq) / widthMhz;
    const curve = normalizedDistance >= 1 ? FLOOR_DBM : FLOOR_DBM + (peakRssi - FLOOR_DBM) * (1 - normalizedDistance * normalizedDistance);
    points.push([Number(x.toFixed(2)), Number(curve.toFixed(2))]);
  }

  return points;
}

function findChannelForFrequency(frequency: number, bandConfig: BandConfig): number | null {
  let bestChannel: number | null = null;
  let bestDistance = Number.POSITIVE_INFINITY;

  for (const channel of bandConfig.channels) {
    const channelFrequency = channelToFrequency(channel);
    const distance = Math.abs(channelFrequency - frequency);
    if (distance < bestDistance) {
      bestDistance = distance;
      bestChannel = channel;
    }
  }

  return bestDistance <= 2 ? bestChannel : null;
}

function MetricTile({
  label,
  value,
  helper,
}: {
  label: string;
  value: string;
  helper?: string;
}) {
  return (
    <div className="rounded-xl border border-border/30 bg-black/30 px-4 py-3">
      <p className="font-mono text-[10px] uppercase tracking-[0.25em] text-muted-foreground">{label}</p>
      <p className="mt-2 font-mono text-xl font-bold text-foreground">{value}</p>
      {helper && <p className="mt-1 text-xs text-muted-foreground">{helper}</p>}
    </div>
  );
}

export function Spectrum() {
  const { beacons, isCapturing, startCapture, stopCapture, error: captureError } = useBeaconCapture();
  const [band, setBand] = useState<BandKey>("2.4");
  const [interfaceName, setInterfaceName] = useState("wlan0");
  const [interfaces, setInterfaces] = useState<NetworkInterface[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selectedBssid, setSelectedBssid] = useState<string | null>(null);

  useEffect(() => {
    let mounted = true;

    void (async () => {
      try {
        const [monitorInterface, wirelessInterfaces] = await Promise.all([
          invoke<string>("get_monitor_interface").catch(() => "wlan0"),
          invoke<NetworkInterface[]>("list_wireless_interfaces").catch(() => []),
        ]);

        if (!mounted) return;

        setInterfaces(wirelessInterfaces);

        if (wirelessInterfaces.some((item) => item.name === monitorInterface)) {
          setInterfaceName(monitorInterface);
        } else if (wirelessInterfaces[0]) {
          setInterfaceName(wirelessInterfaces[0].name);
        } else {
          setInterfaceName(monitorInterface);
        }
      } catch (error) {
        if (!mounted) return;
        setLoadError(toErrorMessage(error));
      }
    })();

    return () => {
      mounted = false;
    };
  }, []);

  const filteredNetworks = useMemo(() => {
    return Array.from(beacons.values())
      .filter((beacon) => bandFromBeacon(beacon) === band)
      .sort((left, right) => right.rssi - left.rssi);
  }, [band, beacons]);

  useEffect(() => {
    if (filteredNetworks.length === 0) {
      setSelectedBssid(null);
      return;
    }

    if (!selectedBssid || !filteredNetworks.some((network) => network.bssid === selectedBssid)) {
      setSelectedBssid(filteredNetworks[0].bssid);
    }
  }, [filteredNetworks, selectedBssid]);

  const selectedNetwork = useMemo(() => {
    return filteredNetworks.find((network) => network.bssid === selectedBssid) ?? null;
  }, [filteredNetworks, selectedBssid]);

  const networkRows = useMemo<NetworkRow[]>(() => {
    return filteredNetworks.map((network) => ({
      ...network,
      display_name: network.ssid || "<hidden>",
      age_label: formatAge(network.timestamp_ms),
      band_label: BAND_CONFIG[band].label,
    }));
  }, [band, filteredNetworks]);

  const hiddenCount = useMemo(() => filteredNetworks.filter((network) => !network.ssid).length, [filteredNetworks]);

  const averageSignal = useMemo(() => {
    if (filteredNetworks.length === 0) return null;
    return Math.round(filteredNetworks.reduce((sum, network) => sum + network.rssi, 0) / filteredNetworks.length);
  }, [filteredNetworks]);

  const channelSummaries = useMemo<ChannelSummary[]>(() => {
    const overlapRadiusMhz = band === "2.4" ? 25 : 40;

    return BAND_CONFIG[band].channels.map((channel) => {
      const frequency = channelToFrequency(channel);
      let penalty = 0;
      let networkCount = 0;
      let strongestRssi: number | null = null;

      for (const network of filteredNetworks) {
        const distance = Math.abs(network.frequency_mhz - frequency);
        if (distance > overlapRadiusMhz) continue;

        networkCount += 1;
        strongestRssi = strongestRssi == null ? network.rssi : Math.max(strongestRssi, network.rssi);

        const proximity = 1 - distance / overlapRadiusMhz;
        const signalWeight = Math.max(0.15, (100 + network.rssi) / 60);
        penalty += proximity * signalWeight;
      }

      return {
        channel,
        frequency,
        score: Math.max(0, Math.min(100, 100 - penalty * 18)),
        networkCount,
        strongestRssi,
      };
    });
  }, [band, filteredNetworks]);

  const recommendedChannels = useMemo(() => {
    return [...channelSummaries]
      .sort((left, right) => right.score - left.score || left.networkCount - right.networkCount || left.channel - right.channel)
      .slice(0, 3);
  }, [channelSummaries]);

  const strongestNetwork = filteredNetworks[0] ?? null;
  const recommendationLabel = recommendedChannels.length > 0
    ? recommendedChannels.map((summary) => `CH ${summary.channel}`).join(" · ")
    : "—";

  const spectrumChartOptions = useMemo(() => {
    const topLabels = new Set(filteredNetworks.slice(0, 6).map((network) => network.bssid));
    const bandConfig = BAND_CONFIG[band];
    const networkByBssid = new Map(filteredNetworks.map((network) => [network.bssid, network] as const));
    const strongestObservedRssi = filteredNetworks.reduce((max, network) => Math.max(max, network.rssi), FLOOR_DBM);
    const spectrumCeiling = Math.max(-20, Math.min(5, Math.ceil((strongestObservedRssi + 8) / 5) * 5));
    const axisLabelFormatter = (value: number) => {
      const rounded = Math.round(value);
      const channel = findChannelForFrequency(value, bandConfig);
      return channel == null ? `${rounded}` : `${rounded}\nCH ${channel}`;
    };

    const envelopeSeries = filteredNetworks.map((network) => {
      const color = hashColor(network.bssid);
      const isSelected = selectedBssid === network.bssid;

      return {
        id: network.bssid,
        name: network.ssid || network.bssid,
        type: "line",
        data: buildEnvelope(network.frequency_mhz, bandConfig.envelopeWidthMhz, network.rssi),
        smooth: true,
        showSymbol: false,
        animation: false,
        z: isSelected ? 4 : 2,
        lineStyle: {
          width: isSelected ? 3.2 : 2.1,
          color: hexToRgba(color, isSelected ? 0.98 : 0.88),
          opacity: 1,
        },
        areaStyle: {
          color: hexToRgba(color, isSelected ? 0.28 : 0.16),
        },
        emphasis: {
          focus: "series",
        },
      };
    });

    const peakSeries = {
      name: "peaks",
      type: "scatter",
      z: 5,
      data: filteredNetworks.map((network) => ({
        value: [network.frequency_mhz, network.rssi],
        bssid: network.bssid,
        itemStyle: {
          color: hashColor(network.bssid),
          shadowBlur: selectedBssid === network.bssid ? 14 : 7,
          shadowColor: hashColor(network.bssid),
        },
        label: {
          show: topLabels.has(network.bssid),
          formatter: network.ssid || `CH ${network.channel}`,
          position: "top",
          color: hashColor(network.bssid),
          fontFamily: "monospace",
          fontSize: 11,
          fontWeight: 700,
          padding: [0, 0, 8, 0],
        },
        symbolSize: selectedBssid === network.bssid ? 12 : 7,
      })),
      tooltip: {
        trigger: "item",
      },
      labelLayout: {
        hideOverlap: true,
      },
      animation: false,
    };

    return {
      backgroundColor: "#050708",
      animation: false,
      tooltip: {
        trigger: "item",
        backgroundColor: "rgba(3, 8, 18, 0.96)",
        borderColor: "rgba(0, 229, 255, 0.25)",
        textStyle: {
          color: "#E6F7FF",
          fontFamily: "monospace",
        },
        formatter: (params: { data?: { bssid?: string }; seriesId?: string; color?: string }) => {
          const bssid = params.data?.bssid ?? params.seriesId;
          const network = bssid ? networkByBssid.get(bssid) : null;
          if (!network) return "";

          const displayName = network.ssid || "<hidden>";
          const distance = estimateDistanceMeters(network.rssi, network.frequency_mhz);

          return `
            <div style="min-width:240px">
              <div style="font-weight:700; letter-spacing:0.08em; color:${params.color ?? "#00E5FF"}; margin-bottom:8px;">
                ${displayName}
              </div>
              <div style="font-size:12px; opacity:0.95; display:grid; grid-template-columns:auto auto; gap:4px 12px;">
                <span style="color:#7f8ea3;">BSSID</span><span>${network.bssid}</span>
                <span style="color:#7f8ea3;">CHANNEL</span><span>CH ${network.channel}</span>
                <span style="color:#7f8ea3;">FREQ</span><span>${network.frequency_mhz} MHz</span>
                <span style="color:#7f8ea3;">SIGNAL</span><span>${network.rssi} dBm</span>
                <span style="color:#7f8ea3;">VENDOR</span><span>${network.vendor || "Unknown"}</span>
                <span style="color:#7f8ea3;">EST. DIST</span><span>~${distance} m</span>
              </div>
            </div>
          `;
        },
      },
      grid: {
        top: 96,
        right: 24,
        bottom: 78,
        left: 64,
      },
      xAxis: {
        type: "value",
        min: bandConfig.frequencyRange[0],
        max: bandConfig.frequencyRange[1],
        interval: bandConfig.axisTickIntervalMhz,
        name: "Frequency (MHz)",
        nameLocation: "middle",
        nameGap: 50,
        axisLine: {
          lineStyle: {
            color: "rgba(148, 163, 184, 0.3)",
          },
        },
        axisTick: {
          show: true,
        },
        axisLabel: {
          color: "rgba(226, 232, 240, 0.78)",
          fontFamily: "monospace",
          lineHeight: 16,
          margin: 12,
          formatter: axisLabelFormatter,
        },
        splitLine: {
          lineStyle: {
            color: "rgba(148, 163, 184, 0.08)",
            type: "dashed",
          },
        },
      },
      yAxis: {
        type: "value",
        min: FLOOR_DBM,
        max: spectrumCeiling,
        name: "RSSI (dBm)",
        nameLocation: "middle",
        nameGap: 46,
        axisLine: {
          lineStyle: {
            color: "rgba(148, 163, 184, 0.3)",
          },
        },
        axisLabel: {
          color: "rgba(226, 232, 240, 0.65)",
          fontFamily: "monospace",
        },
        splitLine: {
          lineStyle: {
            color: "rgba(148, 163, 184, 0.12)",
            type: "dashed",
          },
        },
      },
      series: [...envelopeSeries, peakSeries],
    };
  }, [band, filteredNetworks, recommendedChannels, selectedBssid]);

  const channelChartOptions = useMemo(() => {
    return {
      backgroundColor: "transparent",
      animation: false,
      grid: {
        top: 12,
        right: 18,
        bottom: 16,
        left: 44,
      },
      tooltip: {
        trigger: "axis",
        axisPointer: {
          type: "shadow",
        },
        backgroundColor: "rgba(3, 8, 18, 0.96)",
        borderColor: "rgba(0, 229, 255, 0.25)",
        textStyle: {
          color: "#E6F7FF",
          fontFamily: "monospace",
        },
        formatter: (items: Array<{ axisValueLabel: string; value: number }>) => {
          const item = items[0];
          const summary = channelSummaries.find((entry) => `CH ${entry.channel}` === item.axisValueLabel);
          if (!summary) return "";

          return `
            <div style="min-width:180px">
              <div style="font-weight:700; letter-spacing:0.08em; margin-bottom:8px;">${item.axisValueLabel}</div>
              <div style="display:grid; grid-template-columns:auto auto; gap:4px 12px;">
                <span style="color:#7f8ea3;">Score</span><span>${summary.score.toFixed(0)}%</span>
                <span style="color:#7f8ea3;">APs</span><span>${summary.networkCount}</span>
                <span style="color:#7f8ea3;">Strongest</span><span>${summary.strongestRssi ?? "—"} dBm</span>
              </div>
            </div>
          `;
        },
      },
      xAxis: {
        type: "value",
        max: 100,
        axisLabel: {
          color: "rgba(226, 232, 240, 0.55)",
          fontFamily: "monospace",
          formatter: "{value}%",
        },
        splitLine: {
          lineStyle: {
            color: "rgba(148, 163, 184, 0.1)",
          },
        },
      },
      yAxis: {
        type: "category",
        data: channelSummaries.map((summary) => `CH ${summary.channel}`),
        axisLabel: {
          color: "rgba(226, 232, 240, 0.65)",
          fontFamily: "monospace",
        },
        axisTick: {
          show: false,
        },
        axisLine: {
          show: false,
        },
      },
      series: [{
        type: "bar",
        barWidth: 12,
        data: channelSummaries.map((summary) => ({
          value: Number(summary.score.toFixed(0)),
          itemStyle: {
            color: summary.score >= 75
              ? "#22c55e"
              : summary.score >= 50
                ? "#facc15"
                : "#ef4444",
            borderRadius: [0, 6, 6, 0],
          },
        })),
        label: {
          show: true,
          position: "right",
          color: "rgba(226, 232, 240, 0.75)",
          fontFamily: "monospace",
          formatter: "{c}%",
        },
      }],
    };
  }, [channelSummaries]);

  const chartEvents = useMemo(() => ({
    click: (params: { data?: { bssid?: string }; seriesId?: string }) => {
      const bssid = params.data?.bssid ?? params.seriesId;
      if (bssid) setSelectedBssid(bssid);
    },
  }), []);

  const networkColumns = useMemo(() => ([
    {
      key: "display_name",
      label: "Network",
      render: (row: NetworkRow) => (
        <div className="min-w-0">
          <div className="truncate font-mono text-sm text-foreground">{row.display_name}</div>
          <div className="truncate text-xs text-muted-foreground">{row.bssid}</div>
        </div>
      ),
    },
    {
      key: "channel",
      label: "CH",
      align: "center" as const,
    },
    {
      key: "frequency_mhz",
      label: "Freq",
      render: (row: NetworkRow) => `${row.frequency_mhz} MHz`,
    },
    {
      key: "rssi",
      label: "Signal",
      render: (row: NetworkRow) => <SignalBar rssi={row.rssi} />,
    },
    {
      key: "vendor",
      label: "Vendor",
      render: (row: NetworkRow) => row.vendor || "—",
    },
    {
      key: "age_label",
      label: "Seen",
      align: "right" as const,
      render: (row: NetworkRow) => row.age_label,
    },
  ]), []);

  const interfaceOptions = interfaces.map((networkInterface) => ({
    value: networkInterface.name,
    label: networkInterface.name,
  }));

  const errorMessage = captureError || loadError;

  const handleToggleCapture = async () => {
    if (isCapturing) {
      await stopCapture();
      return;
    }

    await startCapture(interfaceName);
  };

  return (
    <div className="h-full flex flex-col animate-in fade-in slide-in-from-bottom-4 duration-500">
      <header className="mb-6 flex items-start justify-between gap-6">
        <div>
          <h1 className="text-4xl font-mono font-bold tracking-tight text-foreground">
            <span className="text-glow">SPECTRUM</span>
            <span className="ml-3 text-muted-foreground/60">// {BAND_CONFIG[band].label.toUpperCase()}</span>
          </h1>
          <p className="mt-3 font-mono text-sm uppercase tracking-[0.35em] text-muted-foreground">
            Real-time spectral occupancy analysis
          </p>
        </div>

        <button
          onClick={() => void handleToggleCapture()}
          className={`flex items-center gap-3 rounded-xl border px-6 py-3 font-mono text-sm uppercase tracking-[0.3em] transition-all ${
            isCapturing
              ? "border-destructive/80 bg-destructive/10 text-destructive hover:bg-destructive/15"
              : "border-primary bg-primary/10 text-primary hover:bg-primary/15"
          }`}
        >
          {isCapturing ? <Square className="h-4 w-4" /> : <Play className="h-4 w-4" />}
          {isCapturing ? "Stop Capture" : "Start Capture"}
        </button>
      </header>

      {errorMessage && (
        <div className="mb-4 rounded-xl border border-destructive/40 bg-destructive/10 px-4 py-3 font-mono text-sm text-destructive">
          {errorMessage}
        </div>
      )}

      <div className="flex-1 rounded-2xl border border-border/40 bg-black/25 p-6 backdrop-blur-xl">
        <div className="flex flex-wrap items-center justify-between gap-4 border-b border-border/20 pb-5">
          <div className="flex flex-wrap items-center gap-3">
            {(Object.keys(BAND_CONFIG) as BandKey[]).map((bandKey) => (
              <button
                key={bandKey}
                onClick={() => setBand(bandKey)}
                className={`rounded-lg border px-6 py-2 font-mono text-xs uppercase tracking-[0.35em] transition-all ${
                  band === bandKey
                    ? "border-primary bg-primary/15 text-primary shadow-[0_0_18px_rgba(0,229,255,0.15)]"
                    : "border-border/40 bg-black/30 text-muted-foreground hover:border-primary/40 hover:text-foreground"
                }`}
              >
                {BAND_CONFIG[bandKey].label}
              </button>
            ))}
          </div>

          <div className="flex flex-wrap items-center gap-4">
            {interfaceOptions.length > 0 ? (
              <SelectField
                label="Interface"
                value={interfaceName}
                onChange={setInterfaceName}
                options={interfaceOptions}
                className="min-w-[180px]"
              />
            ) : (
              <InputField
                label="Interface"
                value={interfaceName}
                onChange={setInterfaceName}
                placeholder="wlan0"
                className="min-w-[180px]"
              />
            )}

            <div className="flex items-center gap-3">
              <StatusBadge
                status={isCapturing ? "active" : "inactive"}
                label={isCapturing ? "Live Capture" : "Offline"}
              />
              <div className="font-mono text-xs uppercase tracking-[0.25em] text-muted-foreground">
                {filteredNetworks.length} visible
              </div>
            </div>
          </div>
        </div>

        <div className="mt-5 grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4">
          <MetricTile
            label="Visible networks"
            value={filteredNetworks.length.toString()}
            helper={BAND_CONFIG[band].subtitle}
          />
          <MetricTile
            label="Strongest AP"
            value={strongestNetwork ? `${strongestNetwork.rssi} dBm` : "—"}
            helper={strongestNetwork ? (strongestNetwork.ssid || strongestNetwork.bssid) : "Waiting for capture"}
          />
          <MetricTile
            label="Hidden SSIDs"
            value={hiddenCount.toString()}
            helper={averageSignal != null ? `Average signal ${averageSignal} dBm` : "No signal samples yet"}
          />
          <MetricTile
            label="Best channels"
            value={recommendationLabel}
            helper="Calculated from overlap + signal strength"
          />
        </div>

        <GlassCard className="mt-6 border-border/50 bg-black/95">
          <div className="mb-3 flex items-center justify-between">
            <div>
              <p className="font-mono text-xs uppercase tracking-[0.3em] text-primary">Primary spectrum view</p>
              <p className="mt-1 text-sm text-muted-foreground">
                Frequency-domain envelopes for each observed access point on a black analyzer surface.
              </p>
            </div>
            <div className="flex items-center gap-3 font-mono text-xs uppercase tracking-[0.25em] text-muted-foreground">
              <Activity className="h-4 w-4 text-primary" />
              {isCapturing ? "Scanning" : "Paused"}
            </div>
          </div>

          {filteredNetworks.length === 0 ? (
            <div className="min-h-[480px]">
              <EmptyState
                icon={<Radio className="h-12 w-12" />}
                title={isCapturing ? "Waiting for beacons" : "Spectrum offline"}
                description={isCapturing
                  ? `No ${BAND_CONFIG[band].label} networks have appeared yet on ${interfaceName}.`
                  : "Start capture to populate the spectrum graph and channel intelligence panels."}
              />
            </div>
          ) : (
            <ReactECharts
              option={spectrumChartOptions}
              style={{ height: 540, width: "100%", backgroundColor: "#050708", borderRadius: 16 }}
              onEvents={chartEvents}
              notMerge
            />
          )}
        </GlassCard>

        <div className="mt-6 grid grid-cols-1 gap-6 xl:grid-cols-[minmax(0,1.25fr)_minmax(360px,0.9fr)]">
          <GlassCard className="p-4">
            <div className="mb-4 flex items-center justify-between">
              <div>
                <p className="font-mono text-xs uppercase tracking-[0.3em] text-primary">Channel quality</p>
                <p className="mt-1 text-sm text-muted-foreground">Higher scores mean less overlap pressure in the current band.</p>
              </div>
              <Gauge className="h-5 w-5 text-primary" />
            </div>
            <ReactECharts option={channelChartOptions} style={{ height: 360, width: "100%" }} notMerge />
          </GlassCard>

          <div className="space-y-6">
            <GlassCard className="p-4">
              <div className="mb-4 flex items-center justify-between">
                <div>
                  <p className="font-mono text-xs uppercase tracking-[0.3em] text-primary">Selected network</p>
                  <p className="mt-1 text-sm text-muted-foreground">Click a curve or table row to inspect it.</p>
                </div>
                <Signal className="h-5 w-5 text-primary" />
              </div>

              {selectedNetwork ? (
                <>
                  <div className="flex items-start justify-between gap-4">
                    <div className="min-w-0">
                      <h3 className="truncate font-mono text-lg font-bold text-foreground">
                        {selectedNetwork.ssid || "<hidden>"}
                      </h3>
                      <p className="mt-1 truncate font-mono text-xs uppercase tracking-[0.2em] text-muted-foreground">
                        {selectedNetwork.bssid}
                      </p>
                    </div>
                    <StatusBadge
                      status={selectedNetwork.rssi >= -60 ? "success" : selectedNetwork.rssi >= -75 ? "warning" : "error"}
                      label={`${selectedNetwork.rssi} dBm`}
                      pulse={false}
                    />
                  </div>

                  <div className="mt-4">
                    <SignalBar rssi={selectedNetwork.rssi} />
                  </div>

                  <div className="mt-5 grid grid-cols-2 gap-3 font-mono text-xs">
                    <div className="rounded-lg border border-border/20 bg-black/20 px-3 py-2">
                      <div className="text-muted-foreground">Channel</div>
                      <div className="mt-1 text-foreground">CH {selectedNetwork.channel}</div>
                    </div>
                    <div className="rounded-lg border border-border/20 bg-black/20 px-3 py-2">
                      <div className="text-muted-foreground">Frequency</div>
                      <div className="mt-1 text-foreground">{selectedNetwork.frequency_mhz} MHz</div>
                    </div>
                    <div className="rounded-lg border border-border/20 bg-black/20 px-3 py-2">
                      <div className="text-muted-foreground">Vendor</div>
                      <div className="mt-1 text-foreground">{selectedNetwork.vendor || "Unknown"}</div>
                    </div>
                    <div className="rounded-lg border border-border/20 bg-black/20 px-3 py-2">
                      <div className="text-muted-foreground">Estimated distance</div>
                      <div className="mt-1 text-foreground">~{estimateDistanceMeters(selectedNetwork.rssi, selectedNetwork.frequency_mhz)} m</div>
                    </div>
                    <div className="rounded-lg border border-border/20 bg-black/20 px-3 py-2">
                      <div className="text-muted-foreground">Last seen</div>
                      <div className="mt-1 text-foreground">{formatAge(selectedNetwork.timestamp_ms)}</div>
                    </div>
                    <div className="rounded-lg border border-border/20 bg-black/20 px-3 py-2">
                      <div className="text-muted-foreground">Band</div>
                      <div className="mt-1 text-foreground">{BAND_CONFIG[band].label}</div>
                    </div>
                  </div>
                </>
              ) : (
                <EmptyState
                  icon={<Wifi className="h-12 w-12" />}
                  title="No network selected"
                  description="Start capture or switch bands to inspect the strongest network."
                />
              )}
            </GlassCard>
          </div>
        </div>

        <GlassCard className="mt-6 p-4">
          <div className="mb-4 flex items-center justify-between">
            <div>
              <p className="font-mono text-xs uppercase tracking-[0.3em] text-primary">Observed access points</p>
              <p className="mt-1 text-sm text-muted-foreground">Sorted by signal strength for the selected band.</p>
            </div>
            <div className="font-mono text-xs uppercase tracking-[0.25em] text-muted-foreground">
              {filteredNetworks.length} entries
            </div>
          </div>
          <DataTable
            columns={networkColumns}
            data={networkRows}
            keyField="bssid"
            onRowClick={(row) => setSelectedBssid(String(row.bssid))}
            emptyMessage="No access points seen in this band yet."
            maxHeight="360px"
          />
        </GlassCard>
      </div>
    </div>
  );
}
