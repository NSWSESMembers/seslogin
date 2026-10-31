/**
 * @generated SignedSource<<2a5b7c9d7269d1c971e75230e27e3d89>>
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
    readonly accreditations: ReadonlyArray<{
      readonly note: string | null | undefined;
      readonly number: string | null | undefined;
      readonly status: string | null | undefined;
      readonly type: string;
    }>;
    readonly address: {
      readonly formatted: string | null | undefined;
      readonly line1: string | null | undefined;
      readonly line2: string | null | undefined;
      readonly postcode: string | null | undefined;
      readonly state: string | null | undefined;
      readonly suburb: string | null | undefined;
    } | null | undefined;
    readonly code: string | null | undefined;
    readonly email: string | null | undefined;
    readonly latitude: number | null | undefined;
    readonly longitude: number | null | undefined;
    readonly name: string;
    readonly phone: string | null | undefined;
    readonly postalAddress: {
      readonly formatted: string | null | undefined;
      readonly line1: string | null | undefined;
      readonly line2: string | null | undefined;
      readonly postcode: string | null | undefined;
      readonly state: string | null | undefined;
      readonly suburb: string | null | undefined;
    } | null | undefined;
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
  "name": "latitude",
  "storageKey": null
},
v6 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "longitude",
  "storageKey": null
},
v7 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "sesDirectoryEntityId",
  "storageKey": null
},
v8 = [
  {
    "alias": null,
    "args": null,
    "kind": "ScalarField",
    "name": "line1",
    "storageKey": null
  },
  {
    "alias": null,
    "args": null,
    "kind": "ScalarField",
    "name": "line2",
    "storageKey": null
  },
  {
    "alias": null,
    "args": null,
    "kind": "ScalarField",
    "name": "suburb",
    "storageKey": null
  },
  {
    "alias": null,
    "args": null,
    "kind": "ScalarField",
    "name": "state",
    "storageKey": null
  },
  {
    "alias": null,
    "args": null,
    "kind": "ScalarField",
    "name": "postcode",
    "storageKey": null
  },
  {
    "alias": null,
    "args": null,
    "kind": "ScalarField",
    "name": "formatted",
    "storageKey": null
  }
],
v9 = {
  "alias": null,
  "args": null,
  "concreteType": "LocationAddress",
  "kind": "LinkedField",
  "name": "address",
  "plural": false,
  "selections": (v8/*:: as any*/),
  "storageKey": null
},
v10 = {
  "alias": null,
  "args": null,
  "concreteType": "LocationAddress",
  "kind": "LinkedField",
  "name": "postalAddress",
  "plural": false,
  "selections": (v8/*:: as any*/),
  "storageKey": null
},
v11 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "email",
  "storageKey": null
},
v12 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "phone",
  "storageKey": null
},
v13 = {
  "alias": null,
  "args": null,
  "concreteType": "LocationAccreditation",
  "kind": "LinkedField",
  "name": "accreditations",
  "plural": true,
  "selections": [
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "type",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "status",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "number",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "note",
      "storageKey": null
    }
  ],
  "storageKey": null
},
v14 = {
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
          (v5/*:: as any*/),
          (v6/*:: as any*/),
          (v7/*:: as any*/),
          (v9/*:: as any*/),
          (v10/*:: as any*/),
          (v11/*:: as any*/),
          (v12/*:: as any*/),
          (v13/*:: as any*/)
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
              (v14/*:: as any*/)
            ],
            "storageKey": null
          },
          (v5/*:: as any*/),
          (v6/*:: as any*/),
          (v7/*:: as any*/),
          (v9/*:: as any*/),
          (v10/*:: as any*/),
          (v11/*:: as any*/),
          (v12/*:: as any*/),
          (v13/*:: as any*/),
          (v14/*:: as any*/)
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "a9036a4ef838c204f7443c1d918e741f",
    "id": null,
    "metadata": {},
    "name": "LocationDetailsDialogQuery",
    "operationKind": "query",
    "text": "query LocationDetailsDialogQuery(\n  $id: ID!\n) {\n  location(id: $id) {\n    name\n    sesApiHeadquartersId\n    code\n    zone {\n      name\n      id\n    }\n    latitude\n    longitude\n    sesDirectoryEntityId\n    address {\n      line1\n      line2\n      suburb\n      state\n      postcode\n      formatted\n    }\n    postalAddress {\n      line1\n      line2\n      suburb\n      state\n      postcode\n      formatted\n    }\n    email\n    phone\n    accreditations {\n      type\n      status\n      number\n      note\n    }\n    id\n  }\n}\n"
  }
};
})();

(node as any).hash = "ef97af3fb92d620ed4f79f33b9963df5";

export default node;
