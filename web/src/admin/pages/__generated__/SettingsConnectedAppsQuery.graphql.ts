/**
 * @generated SignedSource<<2c805869e9733405c23f74307bfdc388>>
 * @lightSyntaxTransform
 * @nogrep
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
      (v0/*: any*/),
      {
        "alias": null,
        "args": null,
        "concreteType": "OauthGrant",
        "kind": "LinkedField",
        "name": "oauthGrants",
        "plural": true,
        "selections": [
          (v0/*: any*/),
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
    "selections": (v1/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "SettingsConnectedAppsQuery",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "31a93e4ec04f712e0e60d2f20a312e16",
    "id": null,
    "metadata": {},
    "name": "SettingsConnectedAppsQuery",
    "operationKind": "query",
    "text": "query SettingsConnectedAppsQuery {\n  user {\n    id\n    oauthGrants {\n      id\n      clientName\n      redirectHost\n      createdAt\n      lastUsedAt\n      refreshExpiresAt\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "e342aad0f648f190acf755bea82b562d";

export default node;
