/**
 * @generated SignedSource<<2a68cd2596cb11205524b26f8243d90c>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ActivityLastSeenQuery$variables = {
  location: string;
};
export type ActivityLastSeenQuery$data = {
  readonly location: {
    readonly id: string;
    readonly people: ReadonlyArray<{
      readonly firstName: string;
      readonly id: string;
      readonly lastName: string;
      readonly lastPeriod: {
        readonly endTime: number | null | undefined;
        readonly startTime: number;
      } | null | undefined;
    }>;
  };
};
export type ActivityLastSeenQuery = {
  response: ActivityLastSeenQuery$data;
  variables: ActivityLastSeenQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "location"
  }
],
v1 = [
  {
    "kind": "Variable",
    "name": "id",
    "variableName": "location"
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
  "kind": "ScalarField",
  "name": "firstName",
  "storageKey": null
},
v4 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "lastName",
  "storageKey": null
},
v5 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "startTime",
  "storageKey": null
},
v6 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "endTime",
  "storageKey": null
};
return {
  "fragment": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "ActivityLastSeenQuery",
    "selections": [
      {
        "alias": null,
        "args": (v1/*:: as any*/),
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "location",
        "plural": false,
        "selections": [
          (v2/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "Person",
            "kind": "LinkedField",
            "name": "people",
            "plural": true,
            "selections": [
              (v2/*:: as any*/),
              (v3/*:: as any*/),
              (v4/*:: as any*/),
              {
                "alias": null,
                "args": null,
                "concreteType": "Period",
                "kind": "LinkedField",
                "name": "lastPeriod",
                "plural": false,
                "selections": [
                  (v5/*:: as any*/),
                  (v6/*:: as any*/)
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
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "ActivityLastSeenQuery",
    "selections": [
      {
        "alias": null,
        "args": (v1/*:: as any*/),
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "location",
        "plural": false,
        "selections": [
          (v2/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "Person",
            "kind": "LinkedField",
            "name": "people",
            "plural": true,
            "selections": [
              (v2/*:: as any*/),
              (v3/*:: as any*/),
              (v4/*:: as any*/),
              {
                "alias": null,
                "args": null,
                "concreteType": "Period",
                "kind": "LinkedField",
                "name": "lastPeriod",
                "plural": false,
                "selections": [
                  (v5/*:: as any*/),
                  (v6/*:: as any*/),
                  (v2/*:: as any*/)
                ],
                "storageKey": null
              }
            ],
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "42ebd76490bf8620c8fe251a78d1693e",
    "id": null,
    "metadata": {},
    "name": "ActivityLastSeenQuery",
    "operationKind": "query",
    "text": "query ActivityLastSeenQuery(\n  $location: ID!\n) {\n  location(id: $location) {\n    id\n    people {\n      id\n      firstName\n      lastName\n      lastPeriod {\n        startTime\n        endTime\n        id\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "479db63419379efb0fb26d451a679218";

export default node;
