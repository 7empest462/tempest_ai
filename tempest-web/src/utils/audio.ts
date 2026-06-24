import { useStore } from '../store';

// We share an AudioContext globally, but lazily initialize it upon first use
// since browsers require user interaction before audio can be played.
let audioCtx: AudioContext | null = null;

const getAudioContext = () => {
  if (!audioCtx) {
    audioCtx = new (window.AudioContext || (window as any).webkitAudioContext)();
  }
  if (audioCtx.state === 'suspended') {
    audioCtx.resume();
  }
  return audioCtx;
};

// Generic master volume to keep sounds from being abrasive
const MASTER_VOLUME = 0.15;

const canPlay = () => {
  const mute = useStore.getState().muteSounds;
  return !mute;
};

export const playToolInitiationSound = () => {
  if (!canPlay()) return;
  const ctx = getAudioContext();
  const t = ctx.currentTime;

  const osc = ctx.createOscillator();
  const gain = ctx.createGain();

  osc.type = 'triangle';
  osc.frequency.setValueAtTime(800, t);
  osc.frequency.exponentialRampToValueAtTime(1200, t + 0.1);

  gain.gain.setValueAtTime(0, t);
  gain.gain.linearRampToValueAtTime(MASTER_VOLUME, t + 0.02);
  gain.gain.exponentialRampToValueAtTime(0.01, t + 0.15);

  osc.connect(gain);
  gain.connect(ctx.destination);

  osc.start(t);
  osc.stop(t + 0.2);
};

export const playToolSuccessSound = () => {
  if (!canPlay()) return;
  const ctx = getAudioContext();
  const t = ctx.currentTime;

  // Dual oscillators for a bright chime
  const osc1 = ctx.createOscillator();
  const osc2 = ctx.createOscillator();
  const gain = ctx.createGain();

  osc1.type = 'sine';
  osc2.type = 'sine';

  osc1.frequency.setValueAtTime(880, t); // A5
  osc2.frequency.setValueAtTime(1760, t); // A6

  gain.gain.setValueAtTime(0, t);
  gain.gain.linearRampToValueAtTime(MASTER_VOLUME * 0.8, t + 0.05);
  gain.gain.exponentialRampToValueAtTime(0.01, t + 0.6);

  osc1.connect(gain);
  osc2.connect(gain);
  gain.connect(ctx.destination);

  osc1.start(t);
  osc2.start(t);
  osc1.stop(t + 0.7);
  osc2.stop(t + 0.7);
};

export const playToolErrorSound = () => {
  if (!canPlay()) return;
  const ctx = getAudioContext();
  const t = ctx.currentTime;

  const osc = ctx.createOscillator();
  const gain = ctx.createGain();

  osc.type = 'sawtooth';
  osc.frequency.setValueAtTime(150, t);
  osc.frequency.exponentialRampToValueAtTime(80, t + 0.3);

  gain.gain.setValueAtTime(0, t);
  gain.gain.linearRampToValueAtTime(MASTER_VOLUME, t + 0.02);
  // Stutter effect for glitchy feel
  gain.gain.setValueAtTime(MASTER_VOLUME, t + 0.1);
  gain.gain.setValueAtTime(0, t + 0.11);
  gain.gain.setValueAtTime(MASTER_VOLUME * 0.8, t + 0.15);
  gain.gain.exponentialRampToValueAtTime(0.01, t + 0.4);

  osc.connect(gain);
  gain.connect(ctx.destination);

  osc.start(t);
  osc.stop(t + 0.5);
};

export const playApprovalSound = () => {
  if (!canPlay()) return;
  const ctx = getAudioContext();
  const t = ctx.currentTime;

  // High Sine sweep + Noise Whoosh
  const osc = ctx.createOscillator();
  const oscGain = ctx.createGain();

  osc.type = 'sine';
  osc.frequency.setValueAtTime(1200, t);
  osc.frequency.exponentialRampToValueAtTime(2000, t + 0.3);

  oscGain.gain.setValueAtTime(0, t);
  oscGain.gain.linearRampToValueAtTime(MASTER_VOLUME * 0.5, t + 0.1);
  oscGain.gain.exponentialRampToValueAtTime(0.01, t + 0.6);

  osc.connect(oscGain);
  oscGain.connect(ctx.destination);

  osc.start(t);
  osc.stop(t + 0.7);
};

export const playCommandExecutionSound = () => {
  if (!canPlay()) return;
  const ctx = getAudioContext();
  const t = ctx.currentTime;

  const osc = ctx.createOscillator();
  const gain = ctx.createGain();

  osc.type = 'square';
  osc.frequency.setValueAtTime(150, t);
  osc.frequency.exponentialRampToValueAtTime(40, t + 0.1);

  gain.gain.setValueAtTime(0, t);
  gain.gain.linearRampToValueAtTime(MASTER_VOLUME * 0.8, t + 0.01);
  gain.gain.exponentialRampToValueAtTime(0.01, t + 0.15);

  // Filter to make it a "thunk"
  const filter = ctx.createBiquadFilter();
  filter.type = 'lowpass';
  filter.frequency.value = 400;

  osc.connect(filter);
  filter.connect(gain);
  gain.connect(ctx.destination);

  osc.start(t);
  osc.stop(t + 0.2);
};

let lastTerminalBurst = 0;
export const playTerminalBurstSound = () => {
  if (!canPlay()) return;
  const now = Date.now();
  // Throttle to avoid insane clipping if stream is very fast
  if (now - lastTerminalBurst < 40) return;
  lastTerminalBurst = now;

  const ctx = getAudioContext();
  const t = ctx.currentTime;

  const osc = ctx.createOscillator();
  const gain = ctx.createGain();

  osc.type = 'square';
  // Random high pitch for data rain
  osc.frequency.value = 2000 + Math.random() * 2000;

  gain.gain.setValueAtTime(MASTER_VOLUME * 0.1, t);
  gain.gain.exponentialRampToValueAtTime(0.01, t + 0.05);

  osc.connect(gain);
  gain.connect(ctx.destination);

  osc.start(t);
  osc.stop(t + 0.06);
};

export const playTabSwitchSound = () => {
  if (!canPlay()) return;
  const ctx = getAudioContext();
  const t = ctx.currentTime;

  const osc = ctx.createOscillator();
  const gain = ctx.createGain();

  osc.type = 'sine';
  osc.frequency.setValueAtTime(1000, t);
  osc.frequency.exponentialRampToValueAtTime(600, t + 0.05);

  gain.gain.setValueAtTime(0, t);
  gain.gain.linearRampToValueAtTime(MASTER_VOLUME * 0.4, t + 0.01);
  gain.gain.exponentialRampToValueAtTime(0.01, t + 0.05);

  osc.connect(gain);
  gain.connect(ctx.destination);

  osc.start(t);
  osc.stop(t + 0.06);
};

let lastResizeTime = 0;
export const playPanelResizeSound = () => {
  if (!canPlay()) return;
  const now = Date.now();
  // Throttle aggressively during drag to make a discrete mechanical sound
  if (now - lastResizeTime < 100) return;
  lastResizeTime = now;

  const ctx = getAudioContext();
  const t = ctx.currentTime;

  const osc = ctx.createOscillator();
  const gain = ctx.createGain();

  osc.type = 'triangle';
  osc.frequency.value = 100 + Math.random() * 50;

  gain.gain.setValueAtTime(0, t);
  gain.gain.linearRampToValueAtTime(MASTER_VOLUME * 0.2, t + 0.02);
  gain.gain.exponentialRampToValueAtTime(0.01, t + 0.1);

  osc.connect(gain);
  gain.connect(ctx.destination);

  osc.start(t);
  osc.stop(t + 0.12);
};
