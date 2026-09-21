/**
 * @generated SignedSource<<59d1f8c202ed499e2d3e3b6d572dc4f1>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type UserInfoProviderQuery$variables = Record<PropertyKey, never>;
export type UserInfoProviderQuery$data = {
  readonly environment: {
    readonly gitRev: string;
    readonly isProdDb: boolean;
  };
  readonly user: {
    readonly disaggregateVirtualPeriods: boolean;
    readonly email: string;
    readonly id: string;
    readonly isDev: boolean;
    readonly isSuper: boolean;
    readonly locations: ReadonlyArray<{
      readonly enabled: boolean;
      readonly gamificationEnabled: boolean;
      readonly id: string;
      readonly name: string;
      readonly viewerCanEdit: boolean;
    }>;
    readonly passkeys: ReadonlyArray<{
      readonly __typename: "PasskeyInfo";
    }>;
  };
};
export type UserInfoProviderQuery = {
  response: UserInfoProviderQuery$data;
  variables: UserInfoProviderQuery$variables;
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
        "kind": "ScalarField",
        "name": "email",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "isSuper",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "isDev",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "disaggregateVirtualPeriods",
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
          (v0/*:: as any*/),
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
            "name": "enabled",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "viewerCanEdit",
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
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "PasskeyInfo",
        "kind": "LinkedField",
        "name": "passkeys",
        "plural": true,
        "selections": [
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "__typename",
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ],
    "storageKey": null
  },
  {
    "alias": null,
    "args": null,
    "concreteType": "Environment",
    "kind": "LinkedField",
    "name": "environment",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "gitRev",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "isProdDb",
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
    "name": "UserInfoProviderQuery",
    "selections": (v1/*:: as any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "UserInfoProviderQuery",
    "selections": (v1/*:: as any*/)
  },
  "params": {
    "cacheID": "1427f95540bbbc9d1b8e941a0cd6672e",
    "id": null,
    "metadata": {},
    "name": "UserInfoProviderQuery",
    "operationKind": "query",
    "text": "query UserInfoProviderQuery {\n  user {\n    id\n    email\n    isSuper\n    isDev\n    disaggregateVirtualPeriods\n    locations {\n      id\n      name\n      enabled\n      viewerCanEdit\n      gamificationEnabled\n    }\n    passkeys {\n      __typename\n    }\n  }\n  environment {\n    gitRev\n    isProdDb\n  }\n}\n"
  }
};
})();

(node as any).hash = "f4db9a5d5548d01d66c8082c2e310911";

export default node;
