/**
 * @generated SignedSource<<2c046cb6aa23c00833b4217fac326fea>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ScanSignedInPanelQuery$variables = {
  first: number;
};
export type ScanSignedInPanelQuery$data = {
  readonly session: {
    readonly location: {
      readonly periods: {
        readonly edges: ReadonlyArray<{
          readonly node: {
            readonly guestName: string | null | undefined;
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
export type ScanSignedInPanelQuery = {
  response: ScanSignedInPanelQuery$data;
  variables: ScanSignedInPanelQuery$variables;
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
              "kind": "ScalarField",
              "name": "guestName",
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
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "ScanSignedInPanelQuery",
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
    "name": "ScanSignedInPanelQuery",
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
    "cacheID": "3b169bc43f2a84b944db42de8cb0b195",
    "id": null,
    "metadata": {},
    "name": "ScanSignedInPanelQuery",
    "operationKind": "query",
    "text": "query ScanSignedInPanelQuery(\n  $first: Int!\n) {\n  session {\n    location {\n      periods(onlyActive: true, first: $first) {\n        edges {\n          node {\n            id\n            startTime\n            guestName\n            person {\n              id\n              firstName\n              lastName\n            }\n          }\n        }\n      }\n      id\n    }\n    id\n  }\n}\n"
  }
};
})();

(node as any).hash = "c61ccf2cbcca45bb1876e76458650835";

export default node;
