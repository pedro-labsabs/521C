export type SmartProfileId = "music" | "gaming" | "meeting" | "outdoor" | "focus" | "custom";

export type NoiseUiMode =
  | "off"
  | "anc"
  | "adaptive"
  | "indoor"
  | "commuting"
  | "noisy"
  | "transparency";

export type SmartProfile = {
  id: string;
  name: string;
  description: string;
  builtin: boolean;
  noise: NoiseUiMode;
  ancLevel: number;
  transparencyLevel: number;
  gameMode: boolean;
  eqId: string;
  wearDetection: boolean;
  triggerApp?: string;
};

export const BUILTIN_PROFILES: SmartProfile[] = [
  {
    id: "music",
    name: "Music",
    description: "Quality listening — ANC on, musical EQ, game mode off.",
    builtin: true,
    noise: "anc",
    ancLevel: 2,
    transparencyLevel: 4,
    gameMode: false,
    eqId: "pop",
    wearDetection: true,
  },
  {
    id: "gaming",
    name: "Gaming",
    description: "Low latency and a flatter, punchier curve.",
    builtin: true,
    noise: "anc",
    ancLevel: 2,
    transparencyLevel: 4,
    gameMode: true,
    eqId: "gaming",
    wearDetection: false,
    triggerApp: "game",
  },
  {
    id: "meeting",
    name: "Meeting",
    description: "Voice-forward EQ with light transparency for the room.",
    builtin: true,
    noise: "transparency",
    ancLevel: 1,
    transparencyLevel: 3,
    gameMode: false,
    eqId: "voice",
    wearDetection: true,
  },
  {
    id: "outdoor",
    name: "Outdoor",
    description: "Awareness first. Wind reduction is still protocol-pending.",
    builtin: true,
    noise: "transparency",
    ancLevel: 1,
    transparencyLevel: 5,
    gameMode: false,
    eqId: "flat",
    wearDetection: true,
  },
  {
    id: "focus",
    name: "Focus",
    description: "Strong indoor ANC, notifications kept quiet.",
    builtin: true,
    noise: "indoor",
    ancLevel: 3,
    transparencyLevel: 2,
    gameMode: false,
    eqId: "classical",
    wearDetection: true,
  },
];
