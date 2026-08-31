/**
 * @generated SignedSource<<01bf128b7f5b410f4d600b29a631cc43>>
 * @lightSyntaxTransform
 * @nogrep
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
      readonly id: string;
      readonly name: string;
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
      (v0/*: any*/),
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
            "name": "enabled",
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
    "selections": (v1/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "UserInfoProviderQuery",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "67756f51099cbfc4dd5d9fd6d1b2b1ac",
    "id": null,
    "metadata": {},
    "name": "UserInfoProviderQuery",
    "operationKind": "query",
    "text": "query UserInfoProviderQuery {\n  user {\n    id\n    email\n    isSuper\n    isDev\n    disaggregateVirtualPeriods\n    locations {\n      id\n      name\n      enabled\n    }\n    passkeys {\n      __typename\n    }\n  }\n  environment {\n    gitRev\n    isProdDb\n  }\n}\n"
  }
};
})();

(node as any).hash = "c5fb6d3d2a9a8efc366a280f67069df1";

export default node;
