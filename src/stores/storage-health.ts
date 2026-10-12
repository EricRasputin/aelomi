import { create } from "zustand";
import type { Health } from "../ipc";

type StorageHealthStore = {
  health: Health | null;
  receiveHealth: (health: Health) => void;
};

export const useStorageHealthStore = create<StorageHealthStore>((set) => ({
  health: null,
  receiveHealth: (health) => set({ health }),
}));
