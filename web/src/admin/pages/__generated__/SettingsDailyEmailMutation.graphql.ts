/**
 * @generated SignedSource<<9de89a479679248f866881251420b483>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SettingsDailyEmailMutation$variables = {
  dailyLocationIds: ReadonlyArray<string>;
  weeklyBadgeLocationIds: ReadonlyArray<string>;
};
export type SettingsDailyEmailMutation$data = {
  readonly updateMyEmailConfig: {
    readonly badgeWeeklyDigestLocationIds: ReadonlyArray<string>;
    readonly emailSummaryLocationIds: ReadonlyArray<string>;
    readonly id: string;
  };
};
export type SettingsDailyEmailMutation = {
  response: SettingsDailyEmailMutation$data;
  variables: SettingsDailyEmailMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "dailyLocationIds"
  },
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "weeklyBadgeLocationIds"
  }
],
v1 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "dailyLocationIds",
        "variableName": "dailyLocationIds"
      },
      {
        "kind": "Variable",
        "name": "weeklyBadgeLocationIds",
        "variableName": "weeklyBadgeLocationIds"
      }
    ],
    "concreteType": "User",
    "kind": "LinkedField",
    "name": "updateMyEmailConfig",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "id",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "emailSummaryLocationIds",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "badgeWeeklyDigestLocationIds",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "SettingsDailyEmailMutation",
    "selections": (v1/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "SettingsDailyEmailMutation",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "8ef519ae8f3766f6514ffe5b025763a2",
    "id": null,
    "metadata": {},
    "name": "SettingsDailyEmailMutation",
    "operationKind": "mutation",
    "text": "mutation SettingsDailyEmailMutation(\n  $dailyLocationIds: [String!]!\n  $weeklyBadgeLocationIds: [String!]!\n) {\n  updateMyEmailConfig(dailyLocationIds: $dailyLocationIds, weeklyBadgeLocationIds: $weeklyBadgeLocationIds) {\n    id\n    emailSummaryLocationIds\n    badgeWeeklyDigestLocationIds\n  }\n}\n"
  }
};
})();

(node as any).hash = "7bdd87c6ea0cae4e43b35a3c3d8b38fe";

export default node;
