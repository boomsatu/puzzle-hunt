import React from 'react';

export const AnimatedBackground: React.FC = () => {
  return (
    <div className="fixed inset-0 overflow-hidden pointer-events-none -z-10 bg-[#070b12]">
      {/* Static Cyber Grid — no animation, GPU-friendly */}
      <div className="absolute inset-0 bg-cyber-grid opacity-20" />

      {/* Radial Vignette Mask */}
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_80%_80%_at_50%_20%,transparent_15%,#070b12_85%)]" />

      {/* Static Glow Orbs — static position, will-change: none, GPU-friendly */}
      <div className="absolute -top-32 -left-32 w-[400px] h-[400px] bg-amber-500/10 rounded-full blur-[80px] orb-static" />
      <div className="absolute top-1/3 -right-32 w-[450px] h-[450px] bg-cyan-500/8 rounded-full blur-[90px] orb-static" />
      <div className="absolute -bottom-32 left-1/4 w-[500px] h-[500px] bg-indigo-600/10 rounded-full blur-[80px] orb-static" />
    </div>
  );
};

