/**
 * @generated SignedSource<<b58f94441d0154a6b64b94024cd69608>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ScanScreenQuickPickQuery$variables = {
  personId: string;
};
export type ScanScreenQuickPickQuery$data = {
  readonly person: {
    readonly recentCategories: ReadonlyArray<{
      readonly category: {
        readonly id: string;
      };
    }>;
  };
  readonly session: {
    readonly location: {
      readonly recentCategories: ReadonlyArray<{
        readonly category: {
          readonly id: string;
        };
        readonly recentPeople: ReadonlyArray<{
          readonly firstName: string;
          readonly id: string;
        }>;
      }>;
    };
  };
};
export type ScanScreenQuickPickQuery = {
  response: ScanScreenQuickPickQuery$data;
  variables: ScanScreenQuickPickQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "personId"
  }
],
v1 = [
  {
    "kind": "Literal",
    "name": "limit",
    "value": 6
  }
],
v2 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v3 = {
  "alias": null,
  "args": null,
  "concreteType": "Category",
  "kind": "LinkedField",
  "name": "category",
  "plural": false,
  "selections": [
    (v2/*: any*/)
  ],
  "storageKey": null
},
v4 = {
  "alias": null,
  "args": (v1/*: any*/),
  "concreteType": "LocationRecentCategory",
  "kind": "LinkedField",
  "name": "recentCategories",
  "plural": true,
  "selections": [
    (v3/*: any*/),
    {
      "alias": null,
      "args": null,
      "concreteType": "Person",
      "kind": "LinkedField",
      "name": "recentPeople",
      "plural": true,
      "selections": [
        (v2/*: any*/),
        {
          "alias": null,
          "args": null,
          "kind": "ScalarField",
          "name": "firstName",
          "storageKey": null
        }
      ],
      "storageKey": null
    }
  ],
  "storageKey": "recentCategories(limit:6)"
},
v5 = [
  {
    "kind": "Variable",
    "name": "id",
    "variableName": "personId"
  }
],
v6 = {
  "alias": null,
  "args": (v1/*: any*/),
  "concreteType": "PersonRecentCategory",
  "kind": "LinkedField",
  "name": "recentCategories",
  "plural": true,
  "selections": [
    (v3/*: any*/)
  ],
  "storageKey": "recentCategories(limit:6)"
};
return {
  "fragment": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "ScanScreenQuickPickQuery",
    "selections": [
      {
        "alias": null,
        "args": null,
        "concreteType": "Session",
        "kind": "LinkedField",
        "name": "session",
        "plural": false,
        "selections": [
          {
            "alias": null,
            "args": null,
            "concreteType": "Location",
            "kind": "LinkedField",
            "name": "location",
            "plural": false,
            "selections": [
              (v4/*: any*/)
            ],
            "storageKey": null
          }
        ],
        "storageKey": null
      },
      {
        "alias": null,
        "args": (v5/*: any*/),
        "concreteType": "Person",
        "kind": "LinkedField",
        "name": "person",
        "plural": false,
        "selections": [
          (v6/*: any*/)
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
    "name": "ScanScreenQuickPickQuery",
    "selections": [
      {
        "alias": null,
        "args": null,
        "concreteType": "Session",
        "kind": "LinkedField",
        "name": "session",
        "plural": false,
        "selections": [
          {
            "alias": null,
            "args": null,
            "concreteType": "Location",
            "kind": "LinkedField",
            "name": "location",
            "plural": false,
            "selections": [
              (v4/*: any*/),
              (v2/*: any*/)
            ],
            "storageKey": null
          },
          (v2/*: any*/)
        ],
        "storageKey": null
      },
      {
        "alias": null,
        "args": (v5/*: any*/),
        "concreteType": "Person",
        "kind": "LinkedField",
        "name": "person",
        "plural": false,
        "selections": [
          (v6/*: any*/),
          (v2/*: any*/)
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "f7917c394d7b40de31229d51cde9a256",
    "id": null,
    "metadata": {},
    "name": "ScanScreenQuickPickQuery",
    "operationKind": "query",
    "text": "query ScanScreenQuickPickQuery(\n  $personId: ID!\n) {\n  session {\n    location {\n      recentCategories(limit: 6) {\n        category {\n          id\n        }\n        recentPeople {\n          id\n          firstName\n        }\n      }\n      id\n    }\n    id\n  }\n  person(id: $personId) {\n    recentCategories(limit: 6) {\n      category {\n        id\n      }\n    }\n    id\n  }\n}\n"
  }
};
})();

(node as any).hash = "c2321c65f8c62ea1612613f8516ee4f0";

export default node;
