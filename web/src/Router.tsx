import { BrowserRouter, Routes, Route } from "react-router";

// Home imports
import HomeLayout from "./home/Layout";
import Home from "./home/Home";

// Admin imports
import AdminLayout from "./admin/Layout";
import AdminHome from "./admin/pages/AdminHome";
import LocationsList from "./admin/pages/LocationList";
import LocationsEdit from "./admin/pages/LocationEdit";
import LocationsNew from "./admin/pages/LocationNew";
import UserNew from "./admin/pages/UserNew";
import UserList from "./admin/pages/UserList";
import UserEdit from "./admin/pages/UserEdit";
import CategoryList from "./admin/pages/CategoryList";
import CategoryNew from "./admin/pages/CategoryNew";
import CategoryEdit from "./admin/pages/CategoryEdit";
import NitcGroupList from "./admin/pages/NitcGroupList";
import NitcGroupNew from "./admin/pages/NitcGroupNew";
import NitcGroupEdit from "./admin/pages/NitcGroupEdit";
import MembersList from "./admin/pages/MembersList";
import MembersNew from "./admin/pages/MembersNew";
import MembersEdit from "./admin/pages/MembersEdit";
import SessionsList from "./admin/pages/SessionsList";
import SessionsNew from "./admin/pages/SessionsNew";
import SessionsEdit from "./admin/pages/SessionsEdit";
import ActivityList from "./admin/pages/ActivityList";
import ActivityNew from "./admin/pages/ActivityNew";
import ActivityEdit from "./admin/pages/ActivityEdit";
import ActivityListMember from "./admin/pages/ActivityListMember";
import ActivityCurrent from "./admin/pages/ActivityCurrent";
import ActivityTotals from "./admin/pages/ActivityTotals";
import ActivityBreakdown from "./admin/pages/ActivityBreakdown";
import Reports from "./admin/pages/Reports";
import SettingsPasskeys from "./admin/pages/SettingsPasskeys";
import SettingsDailyEmail from "./admin/pages/SettingsDailyEmail";

// Kiosk imports
import KioskMain from "./kiosk/KioskMain";
import StatusDemo from "./kiosk/pages/StatusDemo";

export default function Router() {
  return (
    <BrowserRouter>
      <Routes>
        {/* Home routes - no auth required */}
        <Route path="/" element={<HomeLayout />}>
          <Route index element={<Home />} />
        </Route>

        {/* Admin routes - auth required at /admin/* */}
        <Route path="/admin" element={<AdminLayout />}>
          <Route index element={<AdminHome />} />
          <Route path="locations">
            <Route index element={<LocationsList />} />
            <Route path="new" element={<LocationsNew />} />
            <Route path=":locationId" element={<LocationsEdit />} />
          </Route>
          <Route path="users">
            <Route index element={<UserList />} />
            <Route path="new" element={<UserNew />} />
            <Route path=":userId" element={<UserEdit />} />
          </Route>
          <Route path="categories">
            <Route index element={<CategoryList />} />
            <Route path="new" element={<CategoryNew />} />
            <Route path="nitc-groups">
              <Route index element={<NitcGroupList />} />
              <Route path="new" element={<NitcGroupNew />} />
              <Route path=":nitcGroupId" element={<NitcGroupEdit />} />
            </Route>
            <Route path=":categoryId" element={<CategoryEdit />} />
          </Route>
          <Route path="members">
            <Route index element={<MembersList />} />
            <Route path="new" element={<MembersNew />} />
            <Route path="activity/:memberId" element={<ActivityListMember />} />
            <Route path=":memberId" element={<MembersEdit />} />
          </Route>
          <Route path="sessions">
            <Route index element={<SessionsList />} />
            <Route path="new" element={<SessionsNew />} />
            <Route path=":sessionId" element={<SessionsEdit />} />
          </Route>
          <Route path="activity">
            <Route index element={<ActivityList />} />
            <Route path="new" element={<ActivityNew />} />
            <Route path="current" element={<ActivityCurrent />} />
            <Route path="totals" element={<ActivityTotals />} />
            <Route path="breakdown" element={<ActivityBreakdown />} />
            <Route path=":periodId" element={<ActivityEdit />} />
          </Route>
          <Route path="reports">
            <Route index element={<Reports />} />
          </Route>
          <Route path="settings">
            <Route index element={<SettingsPasskeys />} />
            <Route path="daily-email" element={<SettingsDailyEmail />} />
          </Route>
          <Route path="*" element={<h1>Not Found</h1>} />
        </Route>

        {/* Kiosk routes - auth required at /kiosk */}
        <Route path="/scan" element={<KioskMain />} />
        <Route path="/scan/status-demo" element={<StatusDemo />} />
        <Route path="/kiosk" element={<KioskMain />} />
        <Route path="/kiosk/:profile" element={<KioskMain />} />

        {/* Catch all */}
        <Route path="*" element={<h1>Not Found</h1>} />
      </Routes>
    </BrowserRouter>
  );
}
