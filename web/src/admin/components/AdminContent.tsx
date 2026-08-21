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
        {/* canRetry: every useLazyLoadQuery reachable through a routed page
            here (and ActivityCategorySelector, the one shared component that
            queries outside its own page's boundary) threads
            useRelayRetryFetchKey() into its call. */}
        <RelayErrorBoundary
          resetKey={location.pathname}
          showDetailsByDefault={isDev}
          canRetry
        >
          <Suspense fallback={<LoadingIndicator />}>{children}</Suspense>
        </RelayErrorBoundary>
      </div>
      <Footer />
    </LocationSelector>
  );
}
