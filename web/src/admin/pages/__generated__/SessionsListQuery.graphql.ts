/**
 * @generated SignedSource<<881c13192f6fc7e75e633daca5cab439>>
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
      readonly clientVersion: string | null | undefined;
      readonly code: string | null | undefined;
      readonly id: string;
      readonly keyEnrolled: boolean;
      readonly keyExpiresAt: number | null | undefined;
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
            "name": "reactivatable",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "keyReleasedAt",
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
    "cacheID": "eabf3af4590ee03d9a50c217462c8a00",
    "id": null,
    "metadata": {},
    "name": "SessionsListQuery",
    "operationKind": "query",
    "text": "query SessionsListQuery(\n  $location: ID!\n) {\n  location(id: $location) {\n    id\n    sessions {\n      id\n      name\n      code\n      lastContact\n      clientVersion\n      keyEnrolled\n      keyExpiresAt\n      reactivatable\n      keyReleasedAt\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "6207d90da5c277bfc704b3097df769ae";

export default node;
