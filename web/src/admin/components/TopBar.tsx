interface TopBarProps {
  username: string;
}

export default function TopBar({ username }: TopBarProps) {
  return (
    <div className="flex items-center justify-between bg-black px-2 py-0.5 text-sm font-bold text-white">
      <a href="/" className="text-white no-underline hover:underline">
        SES Activity
      </a>
      <span>{username}</span>
    </div>
  );
}
