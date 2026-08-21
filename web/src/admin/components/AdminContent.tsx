import { useLocation } from "react-router";
import { Suspense } from "react";
import { useUserInfo } from "./useUserInfo";
import TopBar from "./TopBar";
import TitleBar from "./TitleBar";
import MenuBar from "./MenuBar";
import SubmenuBar from "./SubmenuBar";
import Footer from "./Footer";
import LoadingIndicator from "../../components/LoadingIndicator";
import RelayErrorBoundary from "../../components/RelayErrorBoundary";
import LocationSelector from "./LocationSelector";
interface AdminContentProps {
  children?: React.ReactNode;
  onLogout: () => void;
}

export default function AdminContent({
  children,
  onLogout,
}: AdminContentProps) {
  const location = useLocation();
  const { email, isSuper, isDev } = useUserInfo();

  let displayName = email ?? "Unknown";
  if (isSuper) {
    displayName += " [SUPER]";
  }
  if (isDev) {
    displayName += " [DEV]";
  }

  return (
    <LocationSelector>
      <TopBar username={displayName} />
      <TitleBar />
      <MenuBar onLogout={onLogout} isSuper={isSuper} />
      <SubmenuBar isSuper={isSuper} />

      <div className="bg-surface px-[3%] py-5">
        <RelayErrorBoundary
          resetKey={location.pathname}
          showDetailsByDefault={isDev}
        >
          <Suspense fallback={<LoadingIndicator />}>{children}</Suspense>
        </RelayErrorBoundary>
      </div>
      <Footer />
    </LocationSelector>
  );
}
