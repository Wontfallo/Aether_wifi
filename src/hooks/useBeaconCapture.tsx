/**
 * Aether — useBeaconCapture Hook
 *
 * A React hook that manages the lifecycle of a packet capture session,
 * providing a real-time stream of parsed 802.11 Beacon frames.
 *
 * @example
 * ```tsx
 * function SpectrumView() {
 *   const { beacons, isCapturing, error, startCapture, stopCapture } =
 *     useBeaconCapture();
 *
 *   return (
 *     <div>
 *       <button onClick={() => startCapture('wlan0mon')}>Start</button>
 *       <button onClick={stopCapture}>Stop</button>
 *       {isCapturing && <span>Capturing...</span>}
 *       {error && <span>Error: {error}</span>}
 *       <ul>
 *         {Array.from(beacons.values()).map((b) => (
 *           <li key={b.bssid}>
 *             {b.ssid || '(hidden)'} — CH{b.channel} — {b.rssi}dBm
 *           </li>
 *         ))}
 *       </ul>
 *     </div>
 *   );
 * }
 * ```
 */

import { useCallback, useEffect, useRef, useState, createContext, useContext, ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { BeaconFrame, CaptureStatus } from '../types/capture';

/**
 * Return type of the `useBeaconCapture` hook.
 */
export interface UseBeaconCaptureResult {
    /** Map of BSSID → latest BeaconFrame (deduped, most recent per AP) */
    beacons: Map<string, BeaconFrame>;

    /** Ordered list of all beacon events received (for time-series views) */
    beaconStream: BeaconFrame[];

    /** Whether a capture is currently active */
    isCapturing: boolean;

    /** Error message from the last failed operation, or null */
    error: string | null;

    /** Start capturing on the given interface */
    startCapture: (interfaceName: string) => Promise<void>;

    /** Stop the current capture session */
    stopCapture: () => Promise<void>;
}

/**
 * Hook for managing real-time 802.11 beacon capture.
 *
 * - Deduplicates beacons by BSSID (keeps latest reading)
 * - Provides both a deduplicated map and a raw stream array
 * - Auto-cleans up the Tauri event listener on unmount
 *
 * @param maxStreamLength Max entries to keep in `beaconStream` (default: 1000)
 */
function useBeaconCaptureInternal(maxStreamLength = 1000): UseBeaconCaptureResult {
    const [beacons, setBeacons] = useState<Map<string, BeaconFrame>>(new Map());
    const [beaconStream, setBeaconStream] = useState<BeaconFrame[]>([]);
    const [isCapturing, setIsCapturing] = useState(false);
    const [error, setError] = useState<string | null>(null);

    // Refs for the cleanup function
    const unlistenRef = useRef<UnlistenFn | null>(null);
    const streamRef = useRef<BeaconFrame[]>([]);
    const beaconsRef = useRef<Map<string, BeaconFrame>>(new Map());

    // Throttle state updates to avoid excessive re-renders
    const updateTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const pendingRef = useRef(false);

    const flushUpdates = useCallback(() => {
        setBeacons(new Map(beaconsRef.current));
        setBeaconStream([...streamRef.current]);
        pendingRef.current = false;
        updateTimerRef.current = null;
    }, []);

    const scheduleUpdate = useCallback(() => {
        if (!pendingRef.current) {
            pendingRef.current = true;
            // Batch updates every 250ms to keep the UI smooth at 60fps
            updateTimerRef.current = setTimeout(flushUpdates, 250);
        }
    }, [flushUpdates]);

    const startCapture = useCallback(
        async (interfaceName: string) => {
            try {
                setError(null);

                // Set up the event listener BEFORE starting capture to not miss frames
                const unlisten = await listen<BeaconFrame>('beacon-frame', (event) => {
                    const beacon = event.payload;

                    // Update deduplicated map (keyed by BSSID)
                    beaconsRef.current.set(beacon.bssid, beacon);

                    // Append to stream (ring buffer)
                    streamRef.current.push(beacon);
                    if (streamRef.current.length > maxStreamLength) {
                        streamRef.current = streamRef.current.slice(-maxStreamLength);
                    }

                    scheduleUpdate();
                });

                unlistenRef.current = unlisten;

                // Start the capture on the Rust backend
                const status = await invoke<CaptureStatus>('start_capture', {
                    interfaceName,
                });

                setIsCapturing(status.active);
            } catch (err: unknown) {
                const message =
                    err instanceof Error
                        ? err.message
                        : typeof err === 'string'
                            ? err
                            : JSON.stringify(err);
                setError(message);
                setIsCapturing(false);

                // Clean up the listener if capture failed to start
                if (unlistenRef.current) {
                    unlistenRef.current();
                    unlistenRef.current = null;
                }
            }
        },
        [maxStreamLength, scheduleUpdate]
    );

    const stopCapture = useCallback(async () => {
        try {
            await invoke<CaptureStatus>('stop_capture');
            setIsCapturing(false);
        } catch (err: unknown) {
            const message =
                err instanceof Error
                    ? err.message
                    : typeof err === 'string'
                        ? err
                        : JSON.stringify(err);
            setError(message);
        } finally {
            // Always clean up the event listener
            if (unlistenRef.current) {
                unlistenRef.current();
                unlistenRef.current = null;
            }
            if (updateTimerRef.current) {
                clearTimeout(updateTimerRef.current);
                updateTimerRef.current = null;
            }
        }
    }, []);

    // Cleanup on unmount
    useEffect(() => {
        return () => {
            if (unlistenRef.current) {
                unlistenRef.current();
            }
            if (updateTimerRef.current) {
                clearTimeout(updateTimerRef.current);
            }
        };
    }, []);

    return {
        beacons,
        beaconStream,
        isCapturing,
        error,
        startCapture,
        stopCapture,
    };
}

// --- Context Provider to prevent state loss on tab switch ---

const BeaconCaptureContext = createContext<UseBeaconCaptureResult | null>(null);

export function BeaconCaptureProvider({ children }: { children: ReactNode }) {
    const captureState = useBeaconCaptureInternal();
    return (
        <BeaconCaptureContext.Provider value= { captureState } >
        { children }
        </BeaconCaptureContext.Provider>
    );
}

export function useBeaconCapture() {
    const context = useContext(BeaconCaptureContext);
    if (!context) {
        throw new Error("useBeaconCapture must be used within a BeaconCaptureProvider");
    }
    return context;
}
