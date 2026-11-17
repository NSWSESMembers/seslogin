/**
 * @generated SignedSource<<57f80e57ab5651e3ed2b84bc96838797>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type LocationDetailsDialogQuery$variables = {
  id: string;
};
export type LocationDetailsDialogQuery$data = {
  readonly location: {
    readonly code: string | null | undefined;
    readonly name: string;
    readonly sesApiHeadquartersId: string | null | undefined;
    readonly sesDirectoryEntityId: string | null | undefined;
    readonly zone: {
      readonly name: string;
    } | null | undefined;
  };
};
export type LocationDetailsDialogQuery = {
  response: LocationDetailsDialogQuery$data;
  variables: LocationDetailsDialogQuery$variables;
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
  "name": "sesApiHeadquartersId",
  "storageKey": null
},
v4 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "code",
  "storageKey": null
},
v5 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "sesDirectoryEntityId",
  "storageKey": null
},
v6 = {
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
    "name": "LocationDetailsDialogQuery",
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
          (v3/*:: as any*/),
          (v4/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "Location",
            "kind": "LinkedField",
            "name": "zone",
            "plural": false,
            "selections": [
              (v2/*:: as any*/)
            ],
            "storageKey": null
          },
          (v5/*:: as any*/)
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
    "name": "LocationDetailsDialogQuery",
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
          (v3/*:: as any*/),
          (v4/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "Location",
            "kind": "LinkedField",
            "name": "zone",
            "plural": false,
            "selections": [
              (v2/*:: as any*/),
              (v6/*:: as any*/)
            ],
            "storageKey": null
          },
          (v5/*:: as any*/),
          (v6/*:: as any*/)
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "422213f46faca5bedff0a50dfdc53063",
    "id": null,
    "metadata": {},
    "name": "LocationDetailsDialogQuery",
    "operationKind": "query",
    "text": "query LocationDetailsDialogQuery(\n  $id: ID!\n) {\n  location(id: $id) {\n    name\n    sesApiHeadquartersId\n    code\n    zone {\n      name\n      id\n    }\n    sesDirectoryEntityId\n    id\n  }\n}\n"
  }
};
})();

(node as any).hash = "338cac13ca2a1ce8106271256060c7f4";

export default node;
