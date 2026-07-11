/**
 * @generated SignedSource<<f4299118689dff6dfc44c6fb69509f75>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type StatusQuery$variables = {
  first: number;
};
export type StatusQuery$data = {
  readonly session: {
    readonly location: {
      readonly periods: {
        readonly edges: ReadonlyArray<{
          readonly node: {
            readonly id: string;
            readonly person: {
              readonly firstName: string;
              readonly id: string;
              readonly lastName: string;
            } | null | undefined;
            readonly startTime: number;
          };
        }>;
      };
    };
  };
};
export type StatusQuery = {
  response: StatusQuery$data;
  variables: StatusQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "first"
  }
],
v1 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v2 = {
  "alias": null,
  "args": [
    {
      "kind": "Variable",
      "name": "first",
      "variableName": "first"
    },
    {
      "kind": "Literal",
      "name": "onlyActive",
      "value": true
    }
  ],
  "concreteType": "PeriodConnection",
  "kind": "LinkedField",
  "name": "periods",
  "plural": false,
  "selections": [
    {
      "alias": null,
      "args": null,
      "concreteType": "PeriodEdge",
      "kind": "LinkedField",
      "name": "edges",
      "plural": true,
      "selections": [
        {
          "alias": null,
          "args": null,
          "concreteType": "Period",
          "kind": "LinkedField",
          "name": "node",
          "plural": false,
          "selections": [
            (v1/*: any*/),
            {
              "alias": null,
              "args": null,
              "kind": "ScalarField",
              "name": "startTime",
              "storageKey": null
            },
            {
              "alias": null,
              "args": null,
              "concreteType": "Person",
              "kind": "LinkedField",
              "name": "person",
              "plural": false,
              "selections": [
                (v1/*: any*/),
                {
                  "alias": null,
                  "args": null,
                  "kind": "ScalarField",
                  "name": "firstName",
                  "storageKey": null
                },
                {
                  "alias": null,
                  "args": null,
                  "kind": "ScalarField",
                  "name": "lastName",
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
  ],
  "storageKey": null
};
return {
  "fragment": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "StatusQuery",
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
              (v2/*: any*/)
            ],
            "storageKey": null
          }
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
    "name": "StatusQuery",
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
              (v2/*: any*/),
              (v1/*: any*/)
            ],
            "storageKey": null
          },
          (v1/*: any*/)
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "677b7bca7f607d479a1db65c218a6ea7",
    "id": null,
    "metadata": {},
    "name": "StatusQuery",
    "operationKind": "query",
    "text": "query StatusQuery(\n  $first: Int!\n) {\n  session {\n    location {\n      periods(onlyActive: true, first: $first) {\n        edges {\n          node {\n            id\n            startTime\n            person {\n              id\n              firstName\n              lastName\n            }\n          }\n        }\n      }\n      id\n    }\n    id\n  }\n}\n"
  }
};
})();

(node as any).hash = "485f66f08f3dee54212b7711afd2dee6";

export default node;
