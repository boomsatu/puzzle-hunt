import React from 'react';

export const AnimatedBackground: React.FC = () => {
  return (
    <div className="fixed inset-0 overflow-hidden pointer-events-none -z-10 bg-[#070b12]">
      {/* Animated Cyber Grid (Bergerak perlahan secara diagonal) */}
      <div className="absolute inset-0 bg-cyber-grid opacity-25 animate-grid-drift" />

      {/* Radial Vignette Mask (Membuat pinggiran tetap gelap elegan agar kontras teks tajam) */}
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_80%_80%_at_50%_20%,transparent_15%,#070b12_85%)]" />

      {/* Glowing Orb 1: Amber / Bitcoin Gold (Bergerak mengambang dinamis) */}
      <div className="absolute -top-32 -left-32 w-[350px] sm:w-[500px] h-[350px] sm:h-[500px] bg-amber-500/15 rounded-full blur-[120px] animate-blob-1" />

      {/* Glowing Orb 2: Cyan / Blue Tech (Bergerak di kanan tengah) */}
      <div className="absolute top-1/3 -right-32 w-[350px] sm:w-[550px] h-[350px] sm:h-[550px] bg-cyan-500/12 rounded-full blur-[140px] animate-blob-2" />

      {/* Glowing Orb 3: Purple / Blockchain Crypto Core (Bergerak di bawah) */}
      <div className="absolute -bottom-32 left-1/4 w-[350px] sm:w-[600px] h-[350px] sm:h-[600px] bg-indigo-600/15 rounded-full blur-[130px] animate-blob-3" />

      {/* Data Sparks / Floating Ambient Particles */}
      <div className="absolute inset-0 overflow-hidden">
        <div className="particle particle-1" />
        <div className="particle particle-2" />
        <div className="particle particle-3" />
        <div className="particle particle-4" />
        <div className="particle particle-5" />
        <div className="particle particle-6" />
      </div>
    </div>
  );
};

