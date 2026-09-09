/**
 * @generated SignedSource<<514c0e142b30f6725b18bac6771ae0c9>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SessionsListQuery$variables = {
  location: string;
};
export type SessionsListQuery$data = {
  readonly location: {
    readonly id: string;
    readonly sessions: ReadonlyArray<{
      readonly clientInfo: {
        readonly env: string | null | undefined;
        readonly origin: string | null | undefined;
      } | null | undefined;
      readonly clientVersion: string | null | undefined;
      readonly code: string | null | undefined;
      readonly id: string;
      readonly keyEnrolled: boolean;
      readonly keyExpiresAt: number | null | undefined;
      readonly keyFingerprint: string | null | undefined;
      readonly keyReleasedAt: number | null | undefined;
      readonly lastContact: number | null | undefined;
      readonly name: string;
      readonly reactivatable: boolean;
    }>;
  };
};
export type SessionsListQuery = {
  response: SessionsListQuery$data;
  variables: SessionsListQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "location"
  }
],
v1 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v2 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "location"
      }
    ],
    "concreteType": "Location",
    "kind": "LinkedField",
    "name": "location",
    "plural": false,
    "selections": [
      (v1/*: any*/),
      {
        "alias": null,
        "args": null,
        "concreteType": "Session",
        "kind": "LinkedField",
        "name": "sessions",
        "plural": true,
        "selections": [
          (v1/*: any*/),
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
            "name": "code",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "lastContact",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "clientVersion",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "keyEnrolled",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "keyExpiresAt",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "keyFingerprint",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "reactivatable",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "keyReleasedAt",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "concreteType": "SessionClientInfo",
            "kind": "LinkedField",
            "name": "clientInfo",
            "plural": false,
            "selections": [
              {
                "alias": null,
                "args": null,
                "kind": "ScalarField",
                "name": "env",
                "storageKey": null
              },
              {
                "alias": null,
                "args": null,
                "kind": "ScalarField",
                "name": "origin",
                "storageKey": null
              }
            ],
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
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "SessionsListQuery",
    "selections": (v2/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "SessionsListQuery",
    "selections": (v2/*: any*/)
  },
  "params": {
    "cacheID": "f798ce5b8098ada2e0251fbc3e78929f",
    "id": null,
    "metadata": {},
    "name": "SessionsListQuery",
    "operationKind": "query",
    "text": "query SessionsListQuery(\n  $location: ID!\n) {\n  location(id: $location) {\n    id\n    sessions {\n      id\n      name\n      code\n      lastContact\n      clientVersion\n      keyEnrolled\n      keyExpiresAt\n      keyFingerprint\n      reactivatable\n      keyReleasedAt\n      clientInfo {\n        env\n        origin\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "5ee3f0afd43c90a54e1a8ab05c295410";

export default node;
