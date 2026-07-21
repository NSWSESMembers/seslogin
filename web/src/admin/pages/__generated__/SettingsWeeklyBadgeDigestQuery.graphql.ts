/**
 * @generated SignedSource<<de1ea43b70a160857b1d6d6df57884ad>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SettingsWeeklyBadgeDigestQuery$variables = Record<PropertyKey, never>;
export type SettingsWeeklyBadgeDigestQuery$data = {
  readonly user: {
    readonly badgeWeeklyDigestLocationIds: ReadonlyArray<string>;
    readonly emailSummaryLocationIds: ReadonlyArray<string>;
    readonly id: string;
    readonly locations: ReadonlyArray<{
      readonly gamificationEnabled: boolean;
      readonly id: string;
      readonly name: string;
    }>;
  };
};
export type SettingsWeeklyBadgeDigestQuery = {
  response: SettingsWeeklyBadgeDigestQuery$data;
  variables: SettingsWeeklyBadgeDigestQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v1 = [
  {
    "alias": null,
    "args": null,
    "concreteType": "User",
    "kind": "LinkedField",
    "name": "user",
    "plural": false,
    "selections": [
      (v0/*: any*/),
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
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "locations",
        "plural": true,
        "selections": [
          (v0/*: any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "name",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "gamificationEnabled",
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [],
    "kind": "Fragment",
    "metadata": null,
    "name": "SettingsWeeklyBadgeDigestQuery",
    "selections": (v1/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "SettingsWeeklyBadgeDigestQuery",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "555b30b0700ed2b8554c2c2e6d568427",
    "id": null,
    "metadata": {},
    "name": "SettingsWeeklyBadgeDigestQuery",
    "operationKind": "query",
    "text": "query SettingsWeeklyBadgeDigestQuery {\n  user {\n    id\n    emailSummaryLocationIds\n    badgeWeeklyDigestLocationIds\n    locations {\n      id\n      name\n      gamificationEnabled\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "694fcafd297d256e640ae1848ebab5ab";

export default node;
