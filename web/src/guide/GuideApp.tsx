import { Route, Routes } from "react-router";
import NotFound from "../components/NotFound";
import GuideArticle from "./GuideArticle";
import GuideContents from "./GuideContents";
import GuideHome from "./GuideHome";
import GuideLayout from "./GuideLayout";
import GuidePrint from "./GuidePrint";

/**
 * The user guides, mounted at /guide/*. Public — no sign-in — and static: every
 * page is compiled from web/guide/ at build time (see web/guidePlugin.ts), so
 * nothing here talks to the API.
 */
export default function GuideApp() {
  return (
    <GuideLayout>
      <Routes>
        <Route index element={<GuideHome />} />
        <Route path=":guide" element={<GuideContents />} />
        <Route path=":guide/print" element={<GuidePrint />} />
        <Route path=":guide/:slug" element={<GuideArticle />} />
        <Route path="*" element={<NotFound />} />
      </Routes>
    </GuideLayout>
  );
}
