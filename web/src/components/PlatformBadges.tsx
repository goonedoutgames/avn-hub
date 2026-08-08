export const PLATFORMS = [
  "Windows",
  "Mac",
  "Linux",
  "Android",
  "iOS",
  "Web",
] as const;

export type PlatformName = (typeof PLATFORMS)[number];

type IconProps = { size: number };

/** Minimal monochrome platform glyphs — currentColor, no brand fills. */
function WindowsIcon({ size }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" aria-hidden="true">
      <path
        fill="currentColor"
        d="M1.5 2.2 7.2 1.4v6.1H1.5V2.2Zm6.4-.9 6.6-.9v7H7.9V1.3ZM1.5 8.7H7.2v6.1l-5.7-.9V8.7Zm6.4 0h6.6v7l-6.6-.9V8.7Z"
      />
    </svg>
  );
}

function MacIcon({ size }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" aria-hidden="true">
      <path
        fill="currentColor"
        d="M11.2 4.1c-.7.1-1.5.5-2 .9-.5.4-1 .9-1.3.9-.4 0-.9-.5-1.5-.9-.7-.5-1.4-.8-2.1-.8-1.1 0-2.1.7-2.7 1.7-1.1 2-.3 5 1.1 6.6.5.6 1.1 1.3 1.9 1.2.7 0 1-.4 1.8-.4s1.1.4 1.8.4c.8 0 1.3-.6 1.8-1.2.3-.4.6-.8.8-1.3-2.1-.8-2.4-3.9-.4-5.1-.6-.7-1.5-1.1-2.2-1Zm-.4-1.8c.4-.5.7-1.2.6-1.9-.6.1-1.3.4-1.7.9-.4.4-.7 1.1-.6 1.7.7.1 1.3-.2 1.7-.7Z"
      />
    </svg>
  );
}

function LinuxIcon({ size }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" aria-hidden="true">
      <path
        fill="currentColor"
        d="M8 1.4c-1.4 0-2.4 1.4-2.4 3.1 0 .9.3 1.7.7 2.4l-.8 2.5c-.1.4.1.8.5.9l.9.3c.2.7.6 1.3 1.1 1.7v1.3c0 .3.2.5.5.5h.9c.3 0 .5-.2.5-.5v-1.3c.5-.4.9-1 1.1-1.7l.9-.3c.4-.1.6-.5.5-.9l-.8-2.5c.4-.7.7-1.5.7-2.4C10.4 2.8 9.4 1.4 8 1.4Zm-1.3 3.2a.55.55 0 1 1 0 1.1.55.55 0 0 1 0-1.1Zm2.6 0a.55.55 0 1 1 0 1.1.55.55 0 0 1 0-1.1ZM5.2 12.2c-.5.2-1 .6-1.2 1.1-.1.3.1.6.4.6h1.1c.2 0 .4-.1.5-.3.2-.4.5-.7.9-.9l-.3-.6c-.4-.1-.9 0-1.4.1Zm5.6 0c-.5-.1-1-.2-1.4-.1l-.3.6c.4.2.7.5.9.9.1.2.3.3.5.3h1.1c.3 0 .5-.3.4-.6-.2-.5-.7-.9-1.2-1.1Z"
      />
    </svg>
  );
}

function AndroidIcon({ size }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" aria-hidden="true">
      <path
        fill="currentColor"
        d="M4.2 2.2 3.4.8c-.1-.1 0-.3.1-.3.1-.1.3 0 .3.1L4.7 2c.5-.2 1.1-.4 1.7-.4h.2c.6 0 1.2.1 1.7.4l.9-1.4c.1-.1.2-.2.3-.1.1.1.2.2.1.3L9 2.2c1.1.6 1.8 1.7 1.9 3H2.3c.1-1.3.8-2.4 1.9-3ZM5.1 4a.5.5 0 1 0 0-1 .5.5 0 0 0 0 1Zm3 0a.5.5 0 1 0 0-1 .5.5 0 0 0 0 1ZM2 6.2h9.2c.3 0 .6.3.6.6v4.5c0 .5-.4.9-.9.9h-.5v1.9c0 .3-.3.6-.6.6s-.6-.3-.6-.6v-1.9H4.1v1.9c0 .3-.3.6-.6.6s-.6-.3-.6-.6v-1.9h-.5c-.5 0-.9-.4-.9-.9V6.8c0-.3.3-.6.6-.6Zm-1.2.8c-.3 0-.6.3-.6.6v2.4c0 .3.3.6.6.6s.6-.3.6-.6V7.6c0-.3-.3-.6-.6-.6Zm11.6 0c-.3 0-.6.3-.6.6v2.4c0 .3.3.6.6.6s.6-.3.6-.6V7.6c0-.3-.3-.6-.6-.6Z"
      />
    </svg>
  );
}

function IosIcon({ size }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" aria-hidden="true">
      <path
        fill="currentColor"
        fillRule="evenodd"
        d="M5.2 1h5.6C12 1 13 2 13 3.2v9.6c0 1.2-1 2.2-2.2 2.2H5.2C4 15 3 14 3 12.8V3.2C3 2 4 1 5.2 1Zm0 1.1c-.6 0-1.1.5-1.1 1.1v9.6c0 .6.5 1.1 1.1 1.1h5.6c.6 0 1.1-.5 1.1-1.1V3.2c0-.6-.5-1.1-1.1-1.1H5.2Zm2.1 9.5a.7.7 0 1 1 1.4 0 .7.7 0 0 1-1.4 0Z"
        clipRule="evenodd"
      />
    </svg>
  );
}

function WebIcon({ size }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" aria-hidden="true">
      <path
        fill="currentColor"
        fillRule="evenodd"
        d="M8 1.2a6.8 6.8 0 1 0 0 13.6A6.8 6.8 0 0 0 8 1.2ZM2.4 8a5.6 5.6 0 0 1 .4-2.1h2.3a12 12 0 0 0-.2 2.1c0 .7.1 1.4.2 2.1H2.8A5.6 5.6 0 0 1 2.4 8Zm1.1 3.3h2c.3.9.7 1.7 1.2 2.3A5.6 5.6 0 0 1 3.5 11.3Zm2-6.6h-2a5.6 5.6 0 0 1 3.2-2.3c-.5.6-.9 1.4-1.2 2.3Zm1.3 2.1c0-.7.1-1.4.2-2.1h2.4c.1.7.2 1.4.2 2.1s-.1 1.4-.2 2.1H7c-.1-.7-.2-1.4-.2-2.1Zm3.7-2.1h2a5.6 5.6 0 0 0-3.2-2.3c.5.6.9 1.4 1.2 2.3Zm2 2.1c0 .7-.1 1.4-.2 2.1h-2.3c.1-.7.2-1.4.2-2.1s-.1-1.4-.2-2.1h2.3c.1.7.2 1.4.2 2.1Zm-1.1 3.3h-2c-.3.9-.7 1.7-1.2 2.3a5.6 5.6 0 0 0 3.2-2.3Zm-4.4 2.3c-.5-.6-.9-1.4-1.2-2.3h2c-.3.9-.7 1.7-1.2 2.3h.4Z"
        clipRule="evenodd"
      />
    </svg>
  );
}

function PlatformIcon({ platform, size }: { platform: string; size: number }) {
  switch (platform.toLowerCase()) {
    case "windows":
      return <WindowsIcon size={size} />;
    case "mac":
      return <MacIcon size={size} />;
    case "linux":
      return <LinuxIcon size={size} />;
    case "android":
      return <AndroidIcon size={size} />;
    case "ios":
      return <IosIcon size={size} />;
    case "web":
      return <WebIcon size={size} />;
    default:
      return <WebIcon size={size} />;
  }
}

function platformTone(platform: string): string {
  const p = platform.toLowerCase();
  if (p === "windows") return "platform-windows";
  if (p === "mac") return "platform-mac";
  if (p === "linux") return "platform-linux";
  if (p === "android") return "platform-android";
  if (p === "ios") return "platform-ios";
  if (p === "web") return "platform-web";
  return "platform-other";
}

type Props = {
  platforms?: string[] | null;
  limit?: number;
  className?: string;
  size?: "sm" | "md";
};

export function PlatformBadges({
  platforms,
  limit = 6,
  className = "",
  size = "sm",
}: Props) {
  const list = (platforms ?? []).filter(Boolean).slice(0, limit);
  if (list.length === 0) return null;

  const iconSize = size === "md" ? 13 : 11;

  return (
    <div className={`platform-badge-row ${className}`.trim()}>
      {list.map((p) => (
        <span
          key={p}
          className={`platform-badge platform-${size} ${platformTone(p)}`}
          title={p}
        >
          <PlatformIcon platform={p} size={iconSize} />
          <span>{p}</span>
        </span>
      ))}
    </div>
  );
}
