/**
 * @generated SignedSource<<ffcd35e2b59c8058aba4335944320b45>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SettingsPasskeysQuery$variables = Record<PropertyKey, never>;
export type SettingsPasskeysQuery$data = {
  readonly user: {
    readonly id: string;
    readonly passkeys: ReadonlyArray<{
      readonly createdAt: number;
      readonly id: string;
      readonly lastUsedAt: number | null | undefined;
      readonly name: string;
    }>;
  };
};
export type SettingsPasskeysQuery = {
  response: SettingsPasskeysQuery$data;
  variables: SettingsPasskeysQuery$variables;
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
        "concreteType": "PasskeyInfo",
        "kind": "LinkedField",
        "name": "passkeys",
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
            "name": "createdAt",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "lastUsedAt",
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
    "name": "SettingsPasskeysQuery",
    "selections": (v1/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "SettingsPasskeysQuery",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "310017254f320cbfd0b19c10bed33c2d",
    "id": null,
    "metadata": {},
    "name": "SettingsPasskeysQuery",
    "operationKind": "query",
    "text": "query SettingsPasskeysQuery {\n  user {\n    id\n    passkeys {\n      id\n      name\n      createdAt\n      lastUsedAt\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "2add0c01ce18105fcc2a86f7c5fd1b71";

export default node;
