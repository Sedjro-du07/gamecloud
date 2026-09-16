// Interface sounds, synthesised live with the Web Audio API: no file to
// download, nothing from a CDN. The member can switch them off; the choice
// is remembered in this browser.

const KEY = 'gc-son';
let ctx = null;
let master = null;

export function soundEnabled() {
  try {
    return localStorage.getItem(KEY) !== 'off';
  } catch {
    return true;
  }
}

export function setSoundEnabled(on) {
  try {
    localStorage.setItem(KEY, on ? 'on' : 'off');
  } catch {
    // Storage blocked: the switch still works for this page.
  }
}

function audio() {
  if (!ctx) {
    const Ctor = window.AudioContext || window.webkitAudioContext;
    if (!Ctor) return null;
    ctx = new Ctor();
    master = ctx.createGain();
    master.gain.value = 0.8;
    master.connect(ctx.destination);
  }
  if (ctx.state === 'suspended') ctx.resume().catch(() => {});
  return ctx;
}

function tone(freq, start, dur, { type = 'sine', gain = 0.2, to = null, attack = 0.005 } = {}) {
  const osc = ctx.createOscillator();
  const env = ctx.createGain();
  osc.type = type;
  osc.frequency.setValueAtTime(freq, start);
  if (to) osc.frequency.exponentialRampToValueAtTime(to, start + dur);
  env.gain.setValueAtTime(0.0001, start);
  env.gain.exponentialRampToValueAtTime(gain, start + attack);
  env.gain.exponentialRampToValueAtTime(0.0001, start + dur);
  osc.connect(env);
  env.connect(master);
  osc.start(start);
  osc.stop(start + dur + 0.02);
}

function sweep(start, dur, { gain = 0.08, from = 800, to = 3000 } = {}) {
  const length = Math.max(1, Math.floor(ctx.sampleRate * dur));
  const buffer = ctx.createBuffer(1, length, ctx.sampleRate);
  const data = buffer.getChannelData(0);
  for (let i = 0; i < length; i++) data[i] = Math.random() * 2 - 1;
  const source = ctx.createBufferSource();
  source.buffer = buffer;
  const filter = ctx.createBiquadFilter();
  filter.type = 'bandpass';
  filter.Q.value = 1.2;
  filter.frequency.setValueAtTime(from, start);
  filter.frequency.exponentialRampToValueAtTime(to, start + dur);
  const env = ctx.createGain();
  env.gain.setValueAtTime(0.0001, start);
  env.gain.exponentialRampToValueAtTime(gain, start + dur * 0.3);
  env.gain.exponentialRampToValueAtTime(0.0001, start + dur);
  source.connect(filter);
  filter.connect(env);
  env.connect(master);
  source.start(start);
  source.stop(start + dur);
}

const CUES = {
  // A control clicked: short, dry, high.
  tick: (t) => tone(1400, t, 0.08, { type: 'square', gain: 0.3, to: 900 }),
  // Moving to another screen.
  nav: (t) => tone(520, t, 0.11, { type: 'triangle', gain: 0.3, to: 820 }),
  // The main action of a screen.
  confirm: (t) => {
    tone(660, t, 0.1, { type: 'triangle', gain: 0.3 });
    tone(990, t + 0.07, 0.14, { type: 'triangle', gain: 0.26 });
  },
  // Something opens: a menu, a panel.
  open: (t) => sweep(t, 0.2, { gain: 0.3, from: 600, to: 2600 }),
  // It worked.
  success: (t) => [523.25, 659.25, 783.99, 1046.5].forEach((f, i) =>
    tone(f, t + i * 0.07, 0.22, { type: 'triangle', gain: 0.26 })),
  // XP earned: a charge, then sparkles.
  xp: (t) => {
    tone(440, t, 0.35, { type: 'sawtooth', gain: 0.14, to: 1760 });
    [1318.5, 1760, 2349.3].forEach((f, i) => tone(f, t + 0.25 + i * 0.06, 0.18, { type: 'sine', gain: 0.24 }));
  },
  // A new title, a track joined: a small fanfare.
  levelup: (t) => {
    [392, 523.25, 659.25, 783.99].forEach((f, i) => tone(f, t + i * 0.09, 0.3, { type: 'square', gain: 0.14 }));
    tone(1046.5, t + 0.36, 0.6, { type: 'triangle', gain: 0.28 });
    sweep(t + 0.3, 0.4, { gain: 0.1, from: 2000, to: 8000 });
  },
  // It did not work: low and falling, never harsh.
  error: (t) => {
    tone(220, t, 0.2, { type: 'sawtooth', gain: 0.26, to: 140 });
    tone(180, t + 0.13, 0.22, { type: 'sawtooth', gain: 0.2, to: 110 });
  },
  // Kumo answered.
  message: (t) => {
    tone(880, t, 0.14, { type: 'sine', gain: 0.3 });
    tone(1318.5, t + 0.11, 0.2, { type: 'sine', gain: 0.25 });
  },
};

export function playSound(name) {
  if (!soundEnabled()) return;
  const cue = CUES[name];
  if (!cue) return;
  // Until the member has interacted, browsers keep audio closed: stay
  // silent rather than queue a burst of sounds for later.
  if (navigator.userActivation && !navigator.userActivation.hasBeenActive) return;
  const context = audio();
  if (!context) return;
  try {
    cue(context.currentTime + 0.01);
  } catch {
    // A sound is never worth an error.
  }
}
