// Web Audio API Sound Synthesizer untuk Celebration & Notifikasi
// 100% mandiri tanpa file MP3 eksternal (zero load latency, anti-404, bekerja offline)

let audioCtx: AudioContext | null = null;
let soundEnabled = true;

export function getAudioContext(): AudioContext | null {
  if (typeof window === 'undefined') return null;
  if (!audioCtx) {
    const AudioContextClass = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
    if (AudioContextClass) {
      audioCtx = new AudioContextClass();
    }
  }
  if (audioCtx && audioCtx.state === 'suspended') {
    audioCtx.resume();
  }
  return audioCtx;
}

export function isSoundEnabled(): boolean {
  return soundEnabled;
}

export function toggleSound(): boolean {
  soundEnabled = !soundEnabled;
  if (soundEnabled) {
    playCoinChime();
  }
  return soundEnabled;
}

export function setSoundEnabled(enabled: boolean): void {
  soundEnabled = enabled;
}

/**
 * Memainkan nada harmonik sintetis tunggal dengan envelope ADSR
 */
function playTone(
  ctx: AudioContext,
  freq: number,
  startTime: number,
  duration: number,
  type: OscillatorType = 'triangle',
  volume = 0.25
) {
  try {
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();

    osc.type = type;
    osc.frequency.setValueAtTime(freq, startTime);

    gain.gain.setValueAtTime(0, startTime);
    gain.gain.linearRampToValueAtTime(volume, startTime + 0.04);
    gain.gain.exponentialRampToValueAtTime(0.0001, startTime + duration);

    osc.connect(gain);
    gain.connect(ctx.destination);

    osc.start(startTime);
    osc.stop(startTime + duration);
  } catch {
    // Ignore audio error if context suspended
  }
}

/**
 * Suara Koin Emas / Bitcoin Ding (Mirip arcade coin ding)
 */
export function playCoinChime() {
  if (!soundEnabled) return;
  const ctx = getAudioContext();
  if (!ctx) return;

  const now = ctx.currentTime;
  // B5 -> E6
  playTone(ctx, 987.77, now, 0.45, 'sine', 0.28);
  playTone(ctx, 1318.51, now + 0.08, 0.65, 'triangle', 0.35);
  playTone(ctx, 2637.02, now + 0.08, 0.5, 'sine', 0.15); // sparkle overtone
}

/**
 * Suara langkah timeline berhasil (Affirmative chime)
 */
export function playStepSuccessSound() {
  if (!soundEnabled) return;
  const ctx = getAudioContext();
  if (!ctx) return;

  const now = ctx.currentTime;
  playTone(ctx, 659.25, now, 0.25, 'triangle', 0.2); // E5
  playTone(ctx, 880.0, now + 0.1, 0.4, 'sine', 0.25);  // A5
  playTone(ctx, 1174.66, now + 0.2, 0.55, 'sine', 0.3); // D6
}

/**
 * Suara Fanfare Kemenangan Utama (Jackpot Celebration)
 * Memainkan progresi melodi megah C Major + lonceng kemenangan
 */
export function playJackpotFanfare() {
  if (!soundEnabled) return;
  const ctx = getAudioContext();
  if (!ctx) return;

  const now = ctx.currentTime;

  // 1. Triumphant Brass/Synth arpeggio
  const melody = [
    { note: 523.25, delay: 0.0, dur: 0.22, vol: 0.3 },  // C5
    { note: 659.25, delay: 0.16, dur: 0.22, vol: 0.3 }, // E5
    { note: 783.99, delay: 0.32, dur: 0.25, vol: 0.35 },// G5
    { note: 1046.5, delay: 0.48, dur: 0.7, vol: 0.45 }, // C6 (sustained)
  ];

  melody.forEach((m) => {
    playTone(ctx, m.note, now + m.delay, m.dur, 'triangle', m.vol);
    playTone(ctx, m.note * 0.5, now + m.delay, m.dur, 'sawtooth', m.vol * 0.2); // warm bass layer
    playTone(ctx, m.note * 2, now + m.delay, m.dur * 0.8, 'sine', m.vol * 0.25); // sparkle bell layer
  });

  // 2. Cascade of gold bells / coins arpeggio
  const sparkles = [1318.51, 1567.98, 1760.0, 2093.0, 2637.02];
  sparkles.forEach((freq, idx) => {
    playTone(ctx, freq, now + 0.75 + idx * 0.08, 0.5, 'sine', 0.2);
  });

  // 3. Finale Grand Chord (C Major 9th)
  const finaleChord = [523.25, 659.25, 783.99, 987.77, 1046.5];
  finaleChord.forEach((freq) => {
    playTone(ctx, freq, now + 1.25, 1.4, 'triangle', 0.25);
    playTone(ctx, freq * 1.5, now + 1.25, 1.2, 'sine', 0.15);
  });
}

/**
 * Suara Kemenangan Besar saat Auto-Transfer Swept Sukses
 * Cascade 8 koin beruntun + lonceng kemenangan
 */
export function playSweepSuccessFanfare() {
  if (!soundEnabled) return;
  const ctx = getAudioContext();
  if (!ctx) return;

  const now = ctx.currentTime;

  // Rentetan koin slot machine (8 nada naik cepat)
  const coinSequence = [
    587.33, 659.25, 698.46, 783.99, 880.0, 987.77, 1046.5, 1174.66, 1318.51, 1567.98
  ];

  coinSequence.forEach((freq, idx) => {
    playTone(ctx, freq, now + idx * 0.07, 0.35, 'sine', 0.28);
    playTone(ctx, freq * 1.5, now + idx * 0.07, 0.25, 'triangle', 0.15);
  });

  // Penutup gong / lonceng megah di akhir
  const finishTime = now + coinSequence.length * 0.07 + 0.05;
  playTone(ctx, 1046.5, finishTime, 1.8, 'sine', 0.4);
  playTone(ctx, 2093.0, finishTime, 1.6, 'triangle', 0.3);
  playTone(ctx, 523.25, finishTime, 2.0, 'triangle', 0.35);
}

