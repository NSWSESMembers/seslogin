/**
 * @generated SignedSource<<fe711146976d8c202087cea0389cb4ba>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SettingsWeeklyBadgeDigestMutation$variables = {
  dailyLocationIds: ReadonlyArray<string>;
  weeklyBadgeLocationIds: ReadonlyArray<string>;
};
export type SettingsWeeklyBadgeDigestMutation$data = {
  readonly updateMyEmailConfig: {
    readonly badgeWeeklyDigestLocationIds: ReadonlyArray<string>;
    readonly id: string;
  };
};
export type SettingsWeeklyBadgeDigestMutation = {
  response: SettingsWeeklyBadgeDigestMutation$data;
  variables: SettingsWeeklyBadgeDigestMutation$variables;
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
    "name": "SettingsWeeklyBadgeDigestMutation",
    "selections": (v1/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "SettingsWeeklyBadgeDigestMutation",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "8a2fb07991e6fa89dd572e828c552455",
    "id": null,
    "metadata": {},
    "name": "SettingsWeeklyBadgeDigestMutation",
    "operationKind": "mutation",
    "text": "mutation SettingsWeeklyBadgeDigestMutation(\n  $dailyLocationIds: [String!]!\n  $weeklyBadgeLocationIds: [String!]!\n) {\n  updateMyEmailConfig(dailyLocationIds: $dailyLocationIds, weeklyBadgeLocationIds: $weeklyBadgeLocationIds) {\n    id\n    badgeWeeklyDigestLocationIds\n  }\n}\n"
  }
};
})();

(node as any).hash = "b1d3561a1576cb85fe8212001edfc8a6";

export default node;
