import { useTheme } from "@/stores/theme";

interface LogoProps {
  size?: number;
  showWordmark?: boolean;
}

export function Logo({ size = 18, showWordmark = true }: LogoProps) {
  const { resolved } = useTheme();
  const src =
    resolved === "dark"
      ? "/logos/synapse-icon-mono-white.png"
      : "/logos/synapse-icon-mono-black.png";

  return (
    <div className="flex items-center gap-2">
      <img src={src} alt="Synapse" style={{ height: size, width: size }} />
      {showWordmark && (
        <span className="text-[13px] font-semibold tracking-tight text-foreground">Synapse</span>
      )}
    </div>
  );
}
