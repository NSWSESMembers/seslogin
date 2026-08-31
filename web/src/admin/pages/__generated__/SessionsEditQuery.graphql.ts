/**
 * @generated SignedSource<<11dffd21ecf0ff275d282d42a2a55c91>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SessionsEditQuery$variables = {
  id: string;
};
export type SessionsEditQuery$data = {
  readonly session: {
    readonly clientInfo: {
      readonly apiUrl: string | null | undefined;
      readonly clockSkewSecs: number | null | undefined;
      readonly contactFailures: number | null | undefined;
      readonly displayMode: string | null | undefined;
      readonly env: string | null | undefined;
      readonly origin: string | null | undefined;
      readonly pendingVersion: string | null | undefined;
      readonly profile: string | null | undefined;
      readonly screen: string | null | undefined;
      readonly timezone: string | null | undefined;
      readonly updatedAt: number | null | undefined;
      readonly uptimeSecs: number | null | undefined;
      readonly userAgent: string | null | undefined;
    } | null | undefined;
    readonly config: any;
    readonly healthcheckUrl: string | null | undefined;
    readonly name: string;
  };
};
export type SessionsEditQuery = {
  response: SessionsEditQuery$data;
  variables: SessionsEditQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "id"
  }
],
v1 = [
  {
    "kind": "Variable",
    "name": "id",
    "variableName": "id"
  }
],
v2 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "name",
  "storageKey": null
},
v3 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "config",
  "storageKey": null
},
v4 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "healthcheckUrl",
  "storageKey": null
},
v5 = {
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
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "apiUrl",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "profile",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "userAgent",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "screen",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "displayMode",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "timezone",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "clockSkewSecs",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "uptimeSecs",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "pendingVersion",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "contactFailures",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "updatedAt",
      "storageKey": null
    }
  ],
  "storageKey": null
};
return {
  "fragment": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "SessionsEditQuery",
    "selections": [
      {
        "alias": null,
        "args": (v1/*: any*/),
        "concreteType": "Session",
        "kind": "LinkedField",
        "name": "session",
        "plural": false,
        "selections": [
          (v2/*: any*/),
          (v3/*: any*/),
          (v4/*: any*/),
          (v5/*: any*/)
        ],
        "storageKey": null
      }
    ],
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "SessionsEditQuery",
    "selections": [
      {
        "alias": null,
        "args": (v1/*: any*/),
        "concreteType": "Session",
        "kind": "LinkedField",
        "name": "session",
        "plural": false,
        "selections": [
          (v2/*: any*/),
          (v3/*: any*/),
          (v4/*: any*/),
          (v5/*: any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "id",
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "1432940f2edda3ca8e3123c73c53b5de",
    "id": null,
    "metadata": {},
    "name": "SessionsEditQuery",
    "operationKind": "query",
    "text": "query SessionsEditQuery(\n  $id: ID!\n) {\n  session(id: $id) {\n    name\n    config\n    healthcheckUrl\n    clientInfo {\n      env\n      origin\n      apiUrl\n      profile\n      userAgent\n      screen\n      displayMode\n      timezone\n      clockSkewSecs\n      uptimeSecs\n      pendingVersion\n      contactFailures\n      updatedAt\n    }\n    id\n  }\n}\n"
  }
};
})();

(node as any).hash = "b791db551ee1b50d513630d9669f8f13";

export default node;
