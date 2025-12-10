import { useEffect, useRef } from 'react';

/**
 * Custom hook that automatically calls a refresh function every X seconds
 * @param callback - Function to call on each refresh
 * @param intervalMs - Interval in milliseconds (default: 5000)
 * @param enabled - Whether auto-refresh is enabled (default: true)
 */
export function useAutoRefresh(
  callback: () => void | Promise<void>,
  intervalMs: number = 5000,
  enabled: boolean = true
) {
  const savedCallback = useRef(callback);

  // Remember the latest callback
  useEffect(() => {
    savedCallback.current = callback;
  }, [callback]);

  // Set up the interval
  useEffect(() => {
    if (!enabled) return;

    const tick = () => {
      savedCallback.current();
    };

    const id = setInterval(tick, intervalMs);
    return () => clearInterval(id);
  }, [intervalMs, enabled]);
}
