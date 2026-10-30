/**
 * @generated SignedSource<<3dc59bc446977888fe8eb4a25035b5a6>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SettingsConnectedAppsQuery$variables = Record<PropertyKey, never>;
export type SettingsConnectedAppsQuery$data = {
  readonly user: {
    readonly id: string;
    readonly oauthGrants: ReadonlyArray<{
      readonly clientName: string;
      readonly createdAt: number;
      readonly id: string;
      readonly isFirstParty: boolean;
      readonly lastUsedAt: number | null | undefined;
      readonly redirectHost: string;
      readonly refreshExpiresAt: number;
    }>;
  };
};
export type SettingsConnectedAppsQuery = {
  response: SettingsConnectedAppsQuery$data;
  variables: SettingsConnectedAppsQuery$variables;
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
      (v0/*:: as any*/),
      {
        "alias": null,
        "args": null,
        "concreteType": "OauthGrant",
        "kind": "LinkedField",
        "name": "oauthGrants",
        "plural": true,
        "selections": [
          (v0/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "clientName",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "redirectHost",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "isFirstParty",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "createdAt",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "lastUsedAt",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "refreshExpiresAt",
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
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "SettingsConnectedAppsQuery",
    "selections": (v1/*:: as any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "SettingsConnectedAppsQuery",
    "selections": (v1/*:: as any*/)
  },
  "params": {
    "cacheID": "e314d9e3ceccecc8c05a8b179e64bc4c",
    "id": null,
    "metadata": {},
    "name": "SettingsConnectedAppsQuery",
    "operationKind": "query",
    "text": "query SettingsConnectedAppsQuery {\n  user {\n    id\n    oauthGrants {\n      id\n      clientName\n      redirectHost\n      isFirstParty\n      createdAt\n      lastUsedAt\n      refreshExpiresAt\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "9308c20e6a4c9076e1e459950d9c80d4";

export default node;
