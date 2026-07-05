import { useLocation } from "react-router";
import { ErrorBoundary, type FallbackProps } from "react-error-boundary";
import { Suspense } from "react";
import { useUserInfo } from "./useUserInfo";
import TopBar from "./TopBar";
import TitleBar from "./TitleBar";
import MenuBar from "./MenuBar";
import SubmenuBar from "./SubmenuBar";
import Footer from "./Footer";
import LoadingIndicator from "../../components/LoadingIndicator";
import PageErrorFallback from "../../components/PageErrorFallback";
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

      <div className="bg-white px-[3%] py-5">
        <ErrorBoundary
          key={location.pathname}
          fallbackRender={({ error, resetErrorBoundary }: FallbackProps) => (
            <PageErrorFallback
              error={error}
              resetErrorBoundary={resetErrorBoundary}
              showDetailsByDefault={isDev}
            />
          )}
        >
          <Suspense fallback={<LoadingIndicator />}>{children}</Suspense>
        </ErrorBoundary>
      </div>
      <Footer />
    </LocationSelector>
  );
}
