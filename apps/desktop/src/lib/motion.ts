import { useReducedMotion } from "framer-motion";
export { useReducedMotion };

export const ease = [0.2, 0, 0, 1] as const;
export const dur = { fast: 0.15, base: 0.22, slow: 0.3 } as const;

export const staggerList = {
  hidden: {},
  show: { transition: { staggerChildren: 0.04, delayChildren: 0.01 } },
} as const;

export const listItem = {
  hidden: { opacity: 0, y: 8 },
  show: { opacity: 1, y: 0, transition: { duration: 0.22, ease: [0.2, 0, 0, 1] } },
} as const;

export const scaleIn = {
  hidden: { opacity: 0, scale: 0.96, y: 4 },
  show: { opacity: 1, scale: 1, y: 0, transition: { duration: 0.18, ease: [0.2, 0, 0, 1] } },
  exit: { opacity: 0, scale: 0.96, y: 4, transition: { duration: 0.12, ease: [0.2, 0, 0, 1] } },
} as const;
