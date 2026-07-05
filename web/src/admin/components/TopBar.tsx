interface TopBarProps {
  username: string;
}

export default function TopBar({ username }: TopBarProps) {
  return (
    <div className="bg-black px-2.5 py-1 text-[14px] font-bold text-white">
      <div className="float-left">
        <a href="/" className="text-white no-underline hover:underline">
          SES Activity
        </a>
      </div>
      <div className="float-right">
        <span>{username}</span>
      </div>
      <div className="clear-both"></div>
    </div>
  );
}
