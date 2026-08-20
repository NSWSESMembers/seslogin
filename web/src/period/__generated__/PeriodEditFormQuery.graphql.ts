/**
 * @generated SignedSource<<7df902b28a7d466edbe5b97144e926c4>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type PeriodEditFormQuery$variables = Record<PropertyKey, never>;
export type PeriodEditFormQuery$data = {
  readonly categories: ReadonlyArray<{
    readonly enabled: boolean;
    readonly id: string;
    readonly name: string;
  }>;
  readonly linkedPeriod: {
    readonly category: {
      readonly id: string;
      readonly name: string;
    } | null | undefined;
    readonly endTime: number | null | undefined;
    readonly id: string;
    readonly location: {
      readonly name: string;
    };
    readonly person: {
      readonly firstName: string;
      readonly lastName: string;
    } | null | undefined;
    readonly startTime: number;
  };
};
export type PeriodEditFormQuery = {
  response: PeriodEditFormQuery$data;
  variables: PeriodEditFormQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v1 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "startTime",
  "storageKey": null
},
v2 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "endTime",
  "storageKey": null
},
v3 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "name",
  "storageKey": null
},
v4 = {
  "alias": null,
  "args": null,
  "concreteType": "Category",
  "kind": "LinkedField",
  "name": "category",
  "plural": false,
  "selections": [
    (v0/*: any*/),
    (v3/*: any*/)
  ],
  "storageKey": null
},
v5 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "firstName",
  "storageKey": null
},
v6 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "lastName",
  "storageKey": null
},
v7 = {
  "alias": null,
  "args": null,
  "concreteType": "Category",
  "kind": "LinkedField",
  "name": "categories",
  "plural": true,
  "selections": [
    (v0/*: any*/),
    (v3/*: any*/),
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "enabled",
      "storageKey": null
    }
  ],
  "storageKey": null
};
return {
  "fragment": {
    "argumentDefinitions": [],
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "PeriodEditFormQuery",
    "selections": [
      {
        "alias": null,
        "args": null,
        "concreteType": "Period",
        "kind": "LinkedField",
        "name": "linkedPeriod",
        "plural": false,
        "selections": [
          (v0/*: any*/),
          (v1/*: any*/),
          (v2/*: any*/),
          (v4/*: any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "Person",
            "kind": "LinkedField",
            "name": "person",
            "plural": false,
            "selections": [
              (v5/*: any*/),
              (v6/*: any*/)
            ],
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "concreteType": "Location",
            "kind": "LinkedField",
            "name": "location",
            "plural": false,
            "selections": [
              (v3/*: any*/)
            ],
            "storageKey": null
          }
        ],
        "storageKey": null
      },
      (v7/*: any*/)
    ],
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "PeriodEditFormQuery",
    "selections": [
      {
        "alias": null,
        "args": null,
        "concreteType": "Period",
        "kind": "LinkedField",
        "name": "linkedPeriod",
        "plural": false,
        "selections": [
          (v0/*: any*/),
          (v1/*: any*/),
          (v2/*: any*/),
          (v4/*: any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "Person",
            "kind": "LinkedField",
            "name": "person",
            "plural": false,
            "selections": [
              (v5/*: any*/),
              (v6/*: any*/),
              (v0/*: any*/)
            ],
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "concreteType": "Location",
            "kind": "LinkedField",
            "name": "location",
            "plural": false,
            "selections": [
              (v3/*: any*/),
              (v0/*: any*/)
            ],
            "storageKey": null
          }
        ],
        "storageKey": null
      },
      (v7/*: any*/)
    ]
  },
  "params": {
    "cacheID": "93b61c04145ebbe8279e4fe6c6ffa92c",
    "id": null,
    "metadata": {},
    "name": "PeriodEditFormQuery",
    "operationKind": "query",
    "text": "query PeriodEditFormQuery {\n  linkedPeriod {\n    id\n    startTime\n    endTime\n    category {\n      id\n      name\n    }\n    person {\n      firstName\n      lastName\n      id\n    }\n    location {\n      name\n      id\n    }\n  }\n  categories {\n    id\n    name\n    enabled\n  }\n}\n"
  }
};
})();

(node as any).hash = "b31f2bd30dc99c0294addcdf4a35673f";

export default node;
