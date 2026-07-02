import { Routes, Route } from "react-router";

import AdminLayout from "./Layout";
import AdminHome from "./pages/AdminHome";
import LocationsList from "./pages/LocationList";
import LocationsEdit from "./pages/LocationEdit";
import LocationsNew from "./pages/LocationNew";
import UserNew from "./pages/UserNew";
import UserList from "./pages/UserList";
import UserEdit from "./pages/UserEdit";
import CategoryList from "./pages/CategoryList";
import CategoryNew from "./pages/CategoryNew";
import CategoryEdit from "./pages/CategoryEdit";
import NitcGroupList from "./pages/NitcGroupList";
import NitcGroupNew from "./pages/NitcGroupNew";
import NitcGroupEdit from "./pages/NitcGroupEdit";
import MembersList from "./pages/MembersList";
import MembersNew from "./pages/MembersNew";
import MembersEdit from "./pages/MembersEdit";
import MembersBadges from "./pages/MembersBadges";
import SessionsList from "./pages/SessionsList";
import SessionsNew from "./pages/SessionsNew";
import SessionsEdit from "./pages/SessionsEdit";
import ActivityList from "./pages/ActivityList";
import ActivityNew from "./pages/ActivityNew";
import ActivityEdit from "./pages/ActivityEdit";
import ActivityListMember from "./pages/ActivityListMember";
import ActivityCurrent from "./pages/ActivityCurrent";
import ActivityTotals from "./pages/ActivityTotals";
import ActivityBreakdown from "./pages/ActivityBreakdown";
import ActivityLastSeen from "./pages/ActivityLastSeen";
import Reports from "./pages/Reports";
import ReportsBadges from "./pages/ReportsBadges";
import SettingsPasskeys from "./pages/SettingsPasskeys";
import SettingsDailyEmail from "./pages/SettingsDailyEmail";

// Mounted at /admin/* — paths here are relative to /admin.
export default function AdminApp() {
  return (
    <Routes>
      <Route element={<AdminLayout />}>
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
          <Route path="badges/:memberId" element={<MembersBadges />} />
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
          <Route path="last-seen" element={<ActivityLastSeen />} />
          <Route path=":periodId" element={<ActivityEdit />} />
        </Route>
        <Route path="reports">
          <Route index element={<Reports />} />
          <Route path="badges" element={<ReportsBadges />} />
        </Route>
        <Route path="settings">
          <Route index element={<SettingsPasskeys />} />
          <Route path="daily-email" element={<SettingsDailyEmail />} />
        </Route>
        <Route path="*" element={<h1>Not Found</h1>} />
      </Route>
    </Routes>
  );
}
