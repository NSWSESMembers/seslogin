/**
 * @generated SignedSource<<532fb0435f542325cd060c964f503516>>
 * @lightSyntaxTransform
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
    readonly location: {
      readonly gamificationEnabled: boolean;
    };
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
  "kind": "ScalarField",
  "name": "gamificationEnabled",
  "storageKey": null
},
v6 = {
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
},
v7 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
};
return {
  "fragment": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "SessionsEditQuery",
    "selections": [
      {
        "alias": null,
        "args": (v1/*:: as any*/),
        "concreteType": "Session",
        "kind": "LinkedField",
        "name": "session",
        "plural": false,
        "selections": [
          (v2/*:: as any*/),
          (v3/*:: as any*/),
          (v4/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "Location",
            "kind": "LinkedField",
            "name": "location",
            "plural": false,
            "selections": [
              (v5/*:: as any*/)
            ],
            "storageKey": null
          },
          (v6/*:: as any*/)
        ],
        "storageKey": null
      }
    ],
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "SessionsEditQuery",
    "selections": [
      {
        "alias": null,
        "args": (v1/*:: as any*/),
        "concreteType": "Session",
        "kind": "LinkedField",
        "name": "session",
        "plural": false,
        "selections": [
          (v2/*:: as any*/),
          (v3/*:: as any*/),
          (v4/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "Location",
            "kind": "LinkedField",
            "name": "location",
            "plural": false,
            "selections": [
              (v5/*:: as any*/),
              (v7/*:: as any*/)
            ],
            "storageKey": null
          },
          (v6/*:: as any*/),
          (v7/*:: as any*/)
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "4fffafc57b8e82772cc018d58492de7c",
    "id": null,
    "metadata": {},
    "name": "SessionsEditQuery",
    "operationKind": "query",
    "text": "query SessionsEditQuery(\n  $id: ID!\n) {\n  session(id: $id) {\n    name\n    config\n    healthcheckUrl\n    location {\n      gamificationEnabled\n      id\n    }\n    clientInfo {\n      env\n      origin\n      apiUrl\n      profile\n      userAgent\n      screen\n      displayMode\n      timezone\n      clockSkewSecs\n      uptimeSecs\n      pendingVersion\n      contactFailures\n      updatedAt\n    }\n    id\n  }\n}\n"
  }
};
})();

(node as any).hash = "f5f69b15486e90644e21c159d17491e1";

export default node;
