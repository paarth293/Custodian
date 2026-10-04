import { useEffect, useState } from 'react';

const BLINK_MS = 140;

/** Natural blinking: every 2.5–6 s, sometimes a quick double blink. */
export function useBlink(enabled: boolean): boolean {
  const [blinking, setBlinking] = useState(false);

  useEffect(() => {
    if (!enabled) return;
    let timer: ReturnType<typeof setTimeout>;
    const blink = (remaining: number) => {
      setBlinking(true);
      timer = setTimeout(() => {
        setBlinking(false);
        timer = remaining > 1 ? setTimeout(() => blink(remaining - 1), 160) : schedule();
      }, BLINK_MS);
    };
    const schedule = () =>
      setTimeout(() => blink(Math.random() < 0.2 ? 2 : 1), 2500 + Math.random() * 3500);
    timer = schedule();
    return () => {
      clearTimeout(timer);
      setBlinking(false);
    };
  }, [enabled]);

  return enabled && blinking;
}
