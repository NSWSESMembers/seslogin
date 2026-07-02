/**
 * @generated SignedSource<<b944792d031d465c9f405f535b53ae87>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ScanControllerSignOutMutation$variables = {
  categoryId: string;
  endTime: number;
  id: string;
  startTime: number;
};
export type ScanControllerSignOutMutation$data = {
  readonly scanSignOut: {
    readonly awardedBadges: ReadonlyArray<{
      readonly description: string;
      readonly id: string;
      readonly name: string;
      readonly tier: string;
    }>;
    readonly period: {
      readonly category: {
        readonly id: string;
        readonly name: string;
      } | null | undefined;
      readonly endTime: number | null | undefined;
      readonly id: string;
      readonly person: {
        readonly firstName: string;
        readonly id: string;
        readonly lastName: string;
      };
      readonly startTime: number;
    };
  };
};
export type ScanControllerSignOutMutation = {
  response: ScanControllerSignOutMutation$data;
  variables: ScanControllerSignOutMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "categoryId"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "endTime"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "id"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "startTime"
},
v4 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v5 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "name",
  "storageKey": null
},
v6 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "categoryId",
        "variableName": "categoryId"
      },
      {
        "kind": "Variable",
        "name": "endTime",
        "variableName": "endTime"
      },
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      },
      {
        "kind": "Variable",
        "name": "startTime",
        "variableName": "startTime"
      }
    ],
    "concreteType": "ScanSignOutResult",
    "kind": "LinkedField",
    "name": "scanSignOut",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "concreteType": "BadgeAward",
        "kind": "LinkedField",
        "name": "awardedBadges",
        "plural": true,
        "selections": [
          (v4/*: any*/),
          (v5/*: any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "description",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "tier",
            "storageKey": null
          }
        ],
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "Period",
        "kind": "LinkedField",
        "name": "period",
        "plural": false,
        "selections": [
          (v4/*: any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "Person",
            "kind": "LinkedField",
            "name": "person",
            "plural": false,
            "selections": [
              (v4/*: any*/),
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
          },
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
            "name": "endTime",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "concreteType": "Category",
            "kind": "LinkedField",
            "name": "category",
            "plural": false,
            "selections": [
              (v4/*: any*/),
              (v5/*: any*/)
            ],
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
    "argumentDefinitions": [
      (v0/*: any*/),
      (v1/*: any*/),
      (v2/*: any*/),
      (v3/*: any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "ScanControllerSignOutMutation",
    "selections": (v6/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v2/*: any*/),
      (v3/*: any*/),
      (v1/*: any*/),
      (v0/*: any*/)
    ],
    "kind": "Operation",
    "name": "ScanControllerSignOutMutation",
    "selections": (v6/*: any*/)
  },
  "params": {
    "cacheID": "0cab71a3db126f39661f26c59e017924",
    "id": null,
    "metadata": {},
    "name": "ScanControllerSignOutMutation",
    "operationKind": "mutation",
    "text": "mutation ScanControllerSignOutMutation(\n  $id: ID!\n  $startTime: Int!\n  $endTime: Int!\n  $categoryId: ID!\n) {\n  scanSignOut(id: $id, startTime: $startTime, endTime: $endTime, categoryId: $categoryId) {\n    awardedBadges {\n      id\n      name\n      description\n      tier\n    }\n    period {\n      id\n      person {\n        id\n        firstName\n        lastName\n      }\n      startTime\n      endTime\n      category {\n        id\n        name\n      }\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "862becb456a84e27a8f8c60daf82a067";

export default node;
