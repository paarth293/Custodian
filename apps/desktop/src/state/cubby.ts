import { create } from 'zustand';

export type CubbyState = {
  state: number;          // 0 idle, 1 sleeping, 2 listening, 3 thinking, 4 working offline, 5 working online, 6 sandbox, 7 undoing, 8 needs you, 9 done, 10 paused, 11 security alert
  online: boolean;        // drives the body colour, blue (offline) or amber (online)
  badge: number;          // count of items waiting for user
  reduced_motion: boolean; // replaces bouncy motion with gentle fades
  setState: (state: number) => void;
  setOnline: (online: boolean) => void;
  setBadge: (badge: number) => void;
  setReducedMotion: (reduced: boolean) => void;
};

export const useCubbyStore = create<CubbyState>((set) => ({
  state: 0,
  online: false,
  badge: 0,
  reduced_motion: false,
  setState: (state) => set({ state }),
  setOnline: (online) => set({ online }),
  setBadge: (badge) => set({ badge }),
  setReducedMotion: (reduced_motion) => set({ reduced_motion }),
}));
